//! Shared finite two-input normalized-pattern comparison operation.
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub struct PatternComparisonOperation {
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next_request: u32,
    received: [bool; 2],
    closed: [bool; 2],
    emitted: bool,
}

impl PatternComparisonOperation {
    /// Construct before Play with the exact Host-admitted input byte bound.
    pub fn new(maximum_input_bytes: u32) -> Self {
        Self {
            maximum_input_bytes,
            pending: None,
            next_request: 0,
            received: [false; 2],
            closed: [false; 2],
            emitted: false,
        }
    }

    pub fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(port @ 0..=1),
                value,
            } if self.pending.is_none() && !self.received[usize::from(port)] => {
                self.received[usize::from(port)] = true;
                let request = RequestId(self.next_request);
                self.next_request = match self.next_request.checked_add(1) {
                    Some(next) => next,
                    None => return fail(FailureCode::StorageExhausted, 253),
                };
                self.pending = Some(request);
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    self.pending = None;
                    return fail(FailureCode::InvalidInput, 254);
                };
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(port),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None)
                        if self.received == [true, true] && !self.emitted =>
                    {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Completed, None, None) => OperationAction::Await,
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 0),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 250),
                }
            }
            OperationInput::Closed {
                port: PortId(port @ 0..=1),
            } if self.pending.is_none() && !self.closed[usize::from(port)] => {
                self.closed[usize::from(port)] = true;
                if self.closed == [true, true] && self.emitted {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 251),
        }
    }

    pub fn advance(&mut self) -> OperationAction {
        if self.closed == [true, true] && self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }
}

impl conduit_kernel::Operation for PatternComparisonOperation {
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

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
