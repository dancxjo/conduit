//! Shared finite kernel lifecycle for named-template commands.

use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub struct TemplateStorageOperation {
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next_request: u32,
    completed_commands: u64,
    maximum_commands: u64,
    closed: bool,
}

impl TemplateStorageOperation {
    pub fn new(maximum_commands: u64, maximum_input_bytes: u32) -> Self {
        Self {
            maximum_input_bytes,
            pending: None,
            next_request: 0,
            completed_commands: 0,
            maximum_commands,
            closed: false,
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
            } if self.pending.is_none()
                && !self.closed
                && self.completed_commands < self.maximum_commands =>
            {
                let request = RequestId(self.next_request);
                self.next_request = match self.next_request.checked_add(1) {
                    Some(next) => next,
                    None => return fail(FailureCode::StorageExhausted, 263),
                };
                self.pending = Some(request);
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    self.pending = None;
                    return fail(FailureCode::InvalidInput, 264);
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
                        self.completed_commands += 1;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 0),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 260),
                }
            }
            OperationInput::Closed { port: PortId(0) }
                if self.pending.is_none() && !self.closed =>
            {
                self.closed = true;
                OperationAction::Complete
            }
            OperationInput::Value {
                port: PortId(0), ..
            } if self.completed_commands >= self.maximum_commands => {
                fail(FailureCode::StorageExhausted, 262)
            }
            _ => fail(FailureCode::InvalidLifecycle, 261),
        }
    }

    pub fn advance(&mut self) -> OperationAction {
        if self.closed {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }
}

impl conduit_kernel::Operation for TemplateStorageOperation {
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
