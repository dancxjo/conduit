use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, Operation, OperationAction,
    OperationInput, PortId, RequestId, ValueRef,
};

pub(super) enum BookOperation {
    Literal {
        value: ValueRef,
        emitted: bool,
    },
    Morse {
        maximum_input_bytes: u32,
        pending: bool,
        emitted: bool,
    },
    Indicator {
        maximum_input_bytes: u32,
        pending: bool,
        complete: bool,
    },
}

impl Operation for BookOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Literal { value, emitted } if !*emitted => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
            Self::Literal { .. } => OperationAction::Complete,
            Self::Morse { .. } | Self::Indicator { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Morse {
                    maximum_input_bytes,
                    pending,
                    emitted,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*pending && !*emitted => {
                *pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, *maximum_input_bytes)
                        .expect("Morse text input was admitted before Play"),
                }
            }
            (
                Self::Indicator {
                    maximum_input_bytes,
                    pending,
                    ..
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*pending => {
                *pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, *maximum_input_bytes)
                        .expect("indicator pattern was admitted before Play"),
                }
            }
            (
                Self::Morse {
                    pending, emitted, ..
                },
                OperationInput::HostOperationCompleted {
                    request: RequestId(0),
                    outcome,
                },
            ) if *pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return fail(1);
                };
                *pending = false;
                *emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            (
                Self::Indicator {
                    pending, complete, ..
                },
                OperationInput::HostOperationCompleted {
                    request: RequestId(0),
                    outcome,
                },
            ) if *pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none()
                && outcome.output.is_none() =>
            {
                *pending = false;
                *complete = true;
                OperationAction::Await
            }
            (Self::Morse { pending, .. }, OperationInput::Closed { port: PortId(0) })
                if !*pending =>
            {
                OperationAction::Complete
            }
            (
                Self::Indicator {
                    pending, complete, ..
                },
                OperationInput::Closed { port: PortId(0) },
            ) if !*pending && *complete => OperationAction::Complete,
            _ => fail(2),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Literal { emitted, .. } => {
                *emitted = true;
                OperationAction::Complete
            }
            Self::Morse { .. } | Self::Indicator { .. } => OperationAction::Await,
        }
    }
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}
