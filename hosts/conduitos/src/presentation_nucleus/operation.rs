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
    LogicInputs {
        input_count: u8,
        seen: u8,
        next_request: u32,
        pending: bool,
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
            Self::LogicInputs { emitted: true, .. } => OperationAction::Complete,
            Self::Transform { .. }
            | Self::LogicInputs { .. }
            | Self::Latest { .. }
            | Self::Tee { .. }
            | Self::Sink { .. } => OperationAction::Await,
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
                Self::LogicInputs {
                    input_count,
                    seen,
                    next_request,
                    pending,
                    emitted,
                },
                OperationInput::Value { port, value },
            ) if !*pending && !*emitted && port.0 < u16::from(*input_count) => {
                let bit = 1_u8 << port.0;
                if *seen & bit != 0 {
                    return invalid(5);
                }
                let Ok(input) =
                    BoundedValueRef::new(value, conduit_core::SCALAR_ENCODED_LEN as u32)
                else {
                    return invalid(5);
                };
                *seen |= bit;
                *pending = true;
                let request = RequestId(*next_request * 4 + u32::from(port.0));
                *next_request += 1;
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
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
                Self::LogicInputs {
                    pending, emitted, ..
                },
                OperationInput::HostOperationCompleted { outcome, .. },
            ) if *pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                *pending = false;
                if let Some(output) = outcome.output {
                    *emitted = true;
                    OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    }
                } else {
                    OperationAction::Await
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
            (Self::Tee { pending, .. }, OperationInput::Closed { port: PortId(0) })
                if pending.is_none() =>
            {
                OperationAction::Complete
            }
            (
                Self::LogicInputs {
                    input_count,
                    seen,
                    pending,
                    ..
                },
                OperationInput::Closed { port },
            ) if !*pending && port.0 < u16::from(*input_count) => {
                let bit = 1_u8 << port.0;
                if *seen & bit == 0 {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
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
            Self::LogicInputs { emitted: true, .. } => OperationAction::Complete,
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
            Self::Transform { .. }
            | Self::LogicInputs { .. }
            | Self::Latest { .. }
            | Self::Tee { .. }
            | Self::Sink { .. } => OperationAction::Await,
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
