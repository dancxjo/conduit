//! Fixed operation state machines for the native Tour fan-out/Morse Play.

use crate::text_kernel_operations::{LiteralOperation, PresentationOperation, UpperOperation};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, Operation, OperationAction, OperationInput, PortId,
    RequestId,
};

const MORSE_REQUEST: RequestId = RequestId(5);
const INDICATOR_REQUEST: RequestId = RequestId(6);
const LEAF_REQUEST: RequestId = RequestId(7);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MorseOperation {
    pub pending: bool,
    pub emitted: bool,
}

impl Operation for MorseOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.emitted => {
                let Ok(input) =
                    BoundedValueRef::new(value, conduit_text::MAXIMUM_MORSE_INPUT_BYTES as u32)
                else {
                    return invalid(60);
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: MORSE_REQUEST,
                    operation: conduit_kernel::HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == MORSE_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return invalid(61);
                };
                self.pending = false;
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == MORSE_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Cancelled =>
            {
                failure(conduit_kernel::FailureCode::Cancelled, 62)
            }
            OperationInput::Closed { port: PortId(0) } if self.emitted && !self.pending => {
                OperationAction::Complete
            }
            _ => invalid(63),
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct IndicatorOperation {
    pub pending: bool,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LeafOperation {
    pub maximum_input_bytes: u32,
    pub pending: bool,
    pub emitted: bool,
}

impl Operation for LeafOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.emitted => {
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    return invalid(80);
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: LEAF_REQUEST,
                    operation: conduit_kernel::HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == LEAF_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return invalid(81);
                };
                self.pending = false;
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == LEAF_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Cancelled =>
            {
                failure(conduit_kernel::FailureCode::Cancelled, 82)
            }
            OperationInput::Closed { port: PortId(0) } if self.emitted && !self.pending => {
                OperationAction::Complete
            }
            _ => invalid(83),
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

impl Operation for IndicatorOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending => {
                let Ok(input) =
                    BoundedValueRef::new(value, conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32)
                else {
                    return invalid(70);
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: INDICATOR_REQUEST,
                    operation: conduit_kernel::HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if request == INDICATOR_REQUEST
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
                if request == INDICATOR_REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Cancelled =>
            {
                failure(conduit_kernel::FailureCode::Cancelled, 71)
            }
            OperationInput::Closed { port: PortId(0) } if self.complete && !self.pending => {
                OperationAction::Complete
            }
            _ => invalid(72),
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TourMorseOperation {
    Literal(LiteralOperation),
    Upper(UpperOperation),
    TextPresentation(PresentationOperation),
    Morse(MorseOperation),
    Indicator(IndicatorOperation),
    Leaf(LeafOperation),
}

impl Operation for TourMorseOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.start(),
            Self::Upper(operation) => operation.start(),
            Self::TextPresentation(operation) => operation.start(),
            Self::Morse(operation) => operation.start(),
            Self::Indicator(operation) => operation.start(),
            Self::Leaf(operation) => operation.start(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.resume(input),
            Self::Upper(operation) => operation.resume(input),
            Self::TextPresentation(operation) => operation.resume(input),
            Self::Morse(operation) => operation.resume(input),
            Self::Indicator(operation) => operation.resume(input),
            Self::Leaf(operation) => operation.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Literal(operation) => operation.advance(),
            Self::Upper(operation) => operation.advance(),
            Self::TextPresentation(operation) => operation.advance(),
            Self::Morse(operation) => operation.advance(),
            Self::Indicator(operation) => operation.advance(),
            Self::Leaf(operation) => operation.advance(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Literal(operation) => operation.cancel(),
            Self::Upper(operation) => operation.cancel(),
            Self::TextPresentation(operation) => operation.cancel(),
            Self::Morse(operation) => operation.cancel(),
            Self::Indicator(operation) => operation.cancel(),
            Self::Leaf(operation) => operation.cancel(),
        }
    }
}

const fn invalid(detail: u16) -> OperationAction {
    failure(conduit_kernel::FailureCode::InvalidLifecycle, detail)
}

const fn failure(code: conduit_kernel::FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure { code, detail })
}
