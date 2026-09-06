//! Shared kernel lifecycle for exact structured selectors, including flow drops.

use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub struct StructuredSelectorOperation {
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next_request: u32,
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
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Completed, None, None) => OperationAction::Await,
                    (HostOperationDisposition::Cancelled, _, _) => OperationAction::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    }),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
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
