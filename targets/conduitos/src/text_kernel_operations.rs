//! Fixed operation state machines for the ordinary bounded text pipeline.

use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, Operation, OperationAction, OperationInput, PortId,
    RequestId, ValueRef,
};

const UPPER_REQUEST: RequestId = RequestId(1);
const PRESENT_REQUEST: RequestId = RequestId(2);
const TIMER_REQUEST: RequestId = RequestId(3);
const TICK_PRESENT_REQUEST: RequestId = RequestId(4);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TimerState {
    Waiting,
    Emitting,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TimerOperation {
    pub wait: BoundedValueRef,
    pub tick: ValueRef,
    pub state: TimerState,
}

impl Operation for TimerOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::RequestHostOperation {
            request: TIMER_REQUEST,
            operation: conduit_kernel::HostOperationId(0),
            input: self.wait,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if request == TIMER_REQUEST
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.state = TimerState::Emitting;
                OperationAction::Emit {
                    port: PortId(0),
                    value: self.tick,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == TIMER_REQUEST
                    && outcome.disposition == HostOperationDisposition::Cancelled =>
            {
                failure(conduit_kernel::FailureCode::Cancelled, 40)
            }
            _ => failure(conduit_kernel::FailureCode::HostOperationFailed, 41),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.state == TimerState::Emitting {
            self.state = TimerState::Complete;
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    fn cancel(&mut self) {
        self.state = TimerState::Cancelled;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TickPresentationOperation {
    pub pending: bool,
    pub complete: bool,
}

impl Operation for TickPresentationOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending => {
                let Ok(input) = BoundedValueRef::new(value, conduit_time::TICK_ENCODED_LEN) else {
                    return invalid(50);
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: TICK_PRESENT_REQUEST,
                    operation: conduit_kernel::HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == TICK_PRESENT_REQUEST
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
                if request == TICK_PRESENT_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Cancelled =>
            {
                failure(conduit_kernel::FailureCode::Cancelled, 51)
            }
            OperationInput::Closed { port: PortId(0) } if self.complete && !self.pending => {
                OperationAction::Complete
            }
            _ => invalid(52),
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

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
                let Ok(input) = BoundedValueRef::new(value, conduit_text::MAX_TEXT_BYTES) else {
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
                let Ok(input) = BoundedValueRef::new(value, conduit_text::MAX_TEXT_BYTES) else {
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
    Timer(TimerOperation),
    TickPresentation(TickPresentationOperation),
}

impl Operation for PlannedOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.start(),
            Self::Upper(operation) => operation.start(),
            Self::Presentation(operation) => operation.start(),
            Self::Timer(operation) => operation.start(),
            Self::TickPresentation(operation) => operation.start(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.resume(input),
            Self::Upper(operation) => operation.resume(input),
            Self::Presentation(operation) => operation.resume(input),
            Self::Timer(operation) => operation.resume(input),
            Self::TickPresentation(operation) => operation.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.advance(),
            Self::Upper(operation) => operation.advance(),
            Self::Presentation(operation) => operation.advance(),
            Self::Timer(operation) => operation.advance(),
            Self::TickPresentation(operation) => operation.advance(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Literal(operation) => operation.cancel(),
            Self::Upper(operation) => operation.cancel(),
            Self::Presentation(operation) => operation.cancel(),
            Self::Timer(operation) => operation.cancel(),
            Self::TickPresentation(operation) => operation.cancel(),
        }
    }
}
