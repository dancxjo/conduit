use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, Operation, OperationAction,
    OperationInput, PortId, RequestId, ValueRef,
};

pub(super) enum NucleusOperation {
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

impl Operation for NucleusOperation {
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
                *pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, *maximum_input_bytes)
                        .expect("portable presentation value is bounded"),
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
                *pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, *maximum_input_bytes)
                        .expect("fixture manifestation value is bounded"),
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
                    return fail(3);
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
            _ => fail(4),
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

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}
