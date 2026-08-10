//! Fixed operation state machines for the ordinary bounded text pipeline.

use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, Operation, OperationAction, OperationInput, PortId,
    RequestId, ValueRef,
};

const UPPER_REQUEST: RequestId = RequestId(1);
const PRESENT_REQUEST: RequestId = RequestId(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LiteralState {
    Emitting,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LiteralOperation {
    pub text: ValueRef,
    pub state: LiteralState,
}

impl Operation for LiteralOperation {
    fn start(&mut self) -> OperationAction {
        self.state = LiteralState::Emitting;
        OperationAction::Emit {
            port: PortId(0),
            value: self.text,
        }
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        invalid(11)
    }

    fn advance(&mut self) -> OperationAction {
        if self.state == LiteralState::Emitting {
            self.state = LiteralState::Complete;
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    fn cancel(&mut self) {
        self.state = LiteralState::Cancelled;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct UpperOperation {
    pub pending: bool,
    pub emitted: bool,
}

impl Operation for UpperOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.emitted => {
                let Ok(input) = BoundedValueRef::new(value, conduit_std_catalog::MAX_TEXT_BYTES)
                else {
                    return invalid(30);
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: UPPER_REQUEST,
                    operation: conduit_kernel::HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == UPPER_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return invalid(31);
                };
                self.pending = false;
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == UPPER_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Cancelled =>
            {
                failure(conduit_kernel::FailureCode::Cancelled, 32)
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == UPPER_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Failed =>
            {
                failure(conduit_kernel::FailureCode::HostOperationFailed, 33)
            }
            OperationInput::Closed { port: PortId(0) } if self.emitted && !self.pending => {
                OperationAction::Complete
            }
            _ => invalid(34),
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PresentationOperation {
    pub pending: bool,
    pub complete: bool,
}

impl Operation for PresentationOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending => {
                let Ok(input) = BoundedValueRef::new(value, conduit_std_catalog::MAX_TEXT_BYTES)
                else {
                    return invalid(20);
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: PRESENT_REQUEST,
                    operation: conduit_kernel::HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == PRESENT_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = false;
                self.complete = true;
                OperationAction::Await
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == PRESENT_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Cancelled =>
            {
                failure(conduit_kernel::FailureCode::Cancelled, 22)
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == PRESENT_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Failed =>
            {
                failure(conduit_kernel::FailureCode::HostOperationFailed, 23)
            }
            OperationInput::Closed { port: PortId(0) } if self.complete && !self.pending => {
                OperationAction::Complete
            }
            _ => invalid(21),
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

const fn invalid(detail: u16) -> OperationAction {
    failure(conduit_kernel::FailureCode::InvalidLifecycle, detail)
}

const fn failure(code: conduit_kernel::FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure { code, detail })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlannedOperation {
    Literal(LiteralOperation),
    Upper(UpperOperation),
    Presentation(PresentationOperation),
}

impl Operation for PlannedOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.start(),
            Self::Upper(operation) => operation.start(),
            Self::Presentation(operation) => operation.start(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.resume(input),
            Self::Upper(operation) => operation.resume(input),
            Self::Presentation(operation) => operation.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.advance(),
            Self::Upper(operation) => operation.advance(),
            Self::Presentation(operation) => operation.advance(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Literal(operation) => operation.cancel(),
            Self::Upper(operation) => operation.cancel(),
            Self::Presentation(operation) => operation.cancel(),
        }
    }
}
