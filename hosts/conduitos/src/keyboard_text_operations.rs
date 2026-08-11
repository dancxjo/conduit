//! Production operation state machines for the ordinary keyboard-text Play.

use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, Operation, OperationAction,
    OperationInput, PortId, RequestId, ValueRef,
};

#[derive(Clone, Copy)]
pub(crate) struct KeyboardOperation {
    pub(crate) empty: ValueRef,
    pub(crate) pending: Option<RequestId>,
    pub(crate) next: u32,
    pub(crate) maximum: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct StreamTransformOperation {
    pending: Option<RequestId>,
    next: u32,
    allows_empty_output: bool,
}

impl StreamTransformOperation {
    pub(crate) const fn new(allows_empty_output: bool) -> Self {
        Self {
            pending: None,
            next: 0,
            allows_empty_output,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PresentationOperation {
    pub(crate) pending: Option<RequestId>,
    pub(crate) next: u32,
}

#[derive(Clone, Copy)]
pub(crate) enum PlannedOperation {
    Keyboard(KeyboardOperation),
    Keymap(StreamTransformOperation),
    Upper(StreamTransformOperation),
    Presentation(PresentationOperation),
}

impl Operation for KeyboardOperation {
    fn start(&mut self) -> OperationAction {
        self.request()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return fail(60);
                };
                self.pending = None;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            _ => fail(60),
        }
    }

    fn advance(&mut self) -> OperationAction {
        self.next += 1;
        if self.next == self.maximum {
            OperationAction::Complete
        } else {
            self.request()
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl KeyboardOperation {
    fn request(&mut self) -> OperationAction {
        let request = RequestId(self.next);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: conduit_kernel::HostOperationId(0),
            input: BoundedValueRef::new(self.empty, 0).expect("empty input is admitted"),
        }
    }
}

impl Operation for StreamTransformOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: conduit_kernel::HostOperationId(0),
                    input: match BoundedValueRef::new(value, value.byte_len) {
                        Ok(value) => value,
                        Err(_) => return fail(61),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.next += 1;
                match outcome.output {
                    Some(output) => OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    },
                    None if self.allows_empty_output => OperationAction::Await,
                    None => fail(62),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => fail(63),
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
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
            } if self.pending.is_none() => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: conduit_kernel::HostOperationId(0),
                    input: match BoundedValueRef::new(value, value.byte_len) {
                        Ok(value) => value,
                        Err(_) => return fail(64),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.next += 1;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => fail(65),
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl Operation for PlannedOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Keyboard(value) => value.start(),
            Self::Keymap(value) | Self::Upper(value) => value.start(),
            Self::Presentation(value) => value.start(),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Keyboard(value) => value.resume(input),
            Self::Keymap(value) | Self::Upper(value) => value.resume(input),
            Self::Presentation(value) => value.resume(input),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Keyboard(value) => value.advance(),
            Self::Keymap(value) | Self::Upper(value) => value.advance(),
            Self::Presentation(value) => value.advance(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Keyboard(value) => value.cancel(),
            Self::Keymap(value) | Self::Upper(value) => value.cancel(),
            Self::Presentation(value) => value.cancel(),
        }
    }
}

const fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
