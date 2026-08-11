use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, Operation, OperationAction,
    OperationInput, PortId, RequestId, ValueRef,
};

pub(super) enum PresentationOperation {
    Source {
        value: ValueRef,
        emitted: bool,
    },
    Transform {
        maximum_input_bytes: u32,
        pending: bool,
        emitted: bool,
    },
    Sink {
        maximum_input_bytes: u32,
        pending: bool,
        complete: bool,
    },
}

impl Operation for PresentationOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { value, emitted } if !*emitted => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
            Self::Source { .. } => OperationAction::Complete,
            Self::Transform { .. } | Self::Sink { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Transform {
                    maximum_input_bytes,
                    pending,
                    emitted,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*pending && !*emitted => {
                let Ok(input) = BoundedValueRef::new(value, *maximum_input_bytes) else {
                    return invalid(1);
                };
                *pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            (
                Self::Sink {
                    maximum_input_bytes,
                    pending,
                    ..
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*pending => {
                let Ok(input) = BoundedValueRef::new(value, *maximum_input_bytes) else {
                    return invalid(2);
                };
                *pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            (
                Self::Transform {
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
                    return invalid(3);
                };
                *pending = false;
                *emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            (
                Self::Sink {
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
            (Self::Transform { pending, .. }, OperationInput::Closed { port: PortId(0) })
                if !*pending =>
            {
                OperationAction::Complete
            }
            (
                Self::Sink {
                    pending, complete, ..
                },
                OperationInput::Closed { port: PortId(0) },
            ) if !*pending && *complete => OperationAction::Complete,
            _ => invalid(4),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { emitted, .. } => {
                *emitted = true;
                OperationAction::Complete
            }
            Self::Transform { .. } | Self::Sink { .. } => OperationAction::Await,
        }
    }
}

fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}
