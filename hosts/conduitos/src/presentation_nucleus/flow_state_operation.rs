//! Operations owned only by the bounded `state/latest -> flow/tee` proof.

use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, Operation, OperationAction,
    OperationInput, PortId, RequestId, ValueRef,
};

pub(super) enum FlowStateOperation {
    Source {
        value: ValueRef,
        emitted: bool,
    },
    Latest {
        held: Option<ValueRef>,
        released: Option<ValueRef>,
        retain_resumed: bool,
    },
    Tee {
        pending: Option<ValueRef>,
        phase: u8,
    },
    Sink {
        pending: bool,
        complete: bool,
    },
}

impl Operation for FlowStateOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { value, emitted } if !*emitted => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
            Self::Source { .. } => OperationAction::Complete,
            _ => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Latest {
                    held,
                    released,
                    retain_resumed,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if value.byte_len == conduit_core::SCALAR_ENCODED_LEN as u32 => {
                *released = held.replace(value);
                *retain_resumed = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            (
                Self::Latest {
                    held,
                    released,
                    retain_resumed,
                },
                OperationInput::Closed { port: PortId(0) },
            ) => {
                *retain_resumed = false;
                *released = held.take();
                OperationAction::Complete
            }
            (
                Self::Tee { pending, phase },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if value.byte_len == conduit_core::SCALAR_ENCODED_LEN as u32 && pending.is_none() => {
                *pending = Some(value);
                *phase = 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            (Self::Tee { pending, .. }, OperationInput::Closed { port: PortId(0) })
                if pending.is_none() =>
            {
                OperationAction::Complete
            }
            (
                Self::Sink { pending, .. },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*pending => {
                let Ok(input) =
                    BoundedValueRef::new(value, conduit_core::SCALAR_ENCODED_LEN as u32)
                else {
                    return invalid(51);
                };
                *pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            (
                Self::Sink { pending, complete },
                OperationInput::HostOperationCompleted {
                    request: RequestId(0),
                    outcome,
                },
            ) if *pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = false;
                *complete = true;
                OperationAction::Await
            }
            (Self::Sink { pending, complete }, OperationInput::Closed { port: PortId(0) })
                if !*pending && *complete =>
            {
                OperationAction::Complete
            }
            _ => invalid(50),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { emitted, .. } => {
                *emitted = true;
                OperationAction::Complete
            }
            Self::Tee {
                pending: Some(value),
                phase: 1,
            } => {
                let value = *value;
                if let Self::Tee { phase, .. } = self {
                    *phase = 2;
                }
                OperationAction::Emit {
                    port: PortId(1),
                    value,
                }
            }
            Self::Tee {
                pending: Some(_),
                phase: 2,
            } => {
                if let Self::Tee { pending, phase } = self {
                    *pending = None;
                    *phase = 0;
                }
                OperationAction::Await
            }
            _ => OperationAction::Await,
        }
    }

    fn retains_resumed_value(&self) -> bool {
        matches!(
            self,
            Self::Latest {
                retain_resumed: true,
                ..
            }
        )
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        match self {
            Self::Latest { released, .. } => released.take(),
            _ => None,
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Latest {
                held,
                released,
                retain_resumed,
            } => {
                *held = None;
                *released = None;
                *retain_resumed = false;
            }
            Self::Tee { pending, phase } => {
                *pending = None;
                *phase = 0;
            }
            _ => {}
        }
    }
}

fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}
