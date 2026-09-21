//! Shared kernel lifecycle for exact structured selectors, including flow drops.

use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, OperationAction,
    OperationInput, PortId, RequestId,
};

pub struct StructuredSelectorOperation {
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next_request: u32,
}

impl<const PORTS: usize> StepOperation<PORTS> for StructuredSelectorOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(143);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed structured selector completion");
                    io.send(PortId(0), output.value)
                        .expect("ready structured selector output");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed dropped structured selection");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    })
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    return StepOutcome::Fail(failure)
                }
                _ => return step_fail(142),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return step_fail(143);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return step_fail(141);
            };
            let request = RequestId(self.next_request);
            let Some(next_request) = self.next_request.checked_add(1) else {
                return step_fail(140);
            };
            io.consume(PortId(0))
                .expect("present structured selector input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("structured selector Host Call");
            self.next_request = next_request;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed structured selector closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn step_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

impl StructuredSelectorOperation {
    pub fn new(maximum_input_bytes: u32) -> Self {
        Self {
            maximum_input_bytes,
            pending: None,
            next_request: 0,
        }
    }

    pub fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let request = RequestId(self.next_request);
                let Some(next_request) = self.next_request.checked_add(1) else {
                    return fail(140);
                };
                self.next_request = next_request;
                self.pending = Some(request);
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    return fail(141);
                };
                OperationAction::RequestHostCall {
                    request,
                    operation: HostCallId(0),
                    input,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None) => OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    },
                    (HostCallDisposition::Completed, None, None) => OperationAction::Await,
                    (HostCallDisposition::Cancelled, _, _) => OperationAction::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    }),
                    (HostCallDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(142),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => fail(143),
        }
    }

    pub fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }
}

impl conduit_kernel::Operation for StructuredSelectorOperation {
    fn start(&mut self) -> OperationAction {
        Self::start(self)
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        Self::resume(self, input)
    }
    fn advance(&mut self) -> OperationAction {
        Self::advance(self)
    }
    fn cancel(&mut self) {
        Self::cancel(self)
    }
}
fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
