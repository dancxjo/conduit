//! Fixed-storage operations for bounded count, toggle, and typed key fan-out.

use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, Operation, OperationAction,
    OperationInput, PortId, RequestId, ValueRef,
};

pub(super) enum PortableStateInputOperation {
    Source {
        value: ValueRef,
        emitted: bool,
    },
    Count {
        values: [ValueRef; 2],
        next: usize,
        initial_emitted: bool,
    },
    Toggle {
        values: [ValueRef; 2],
        next: usize,
        initial_emitted: bool,
    },
    KeyTee {
        pending: Option<ValueRef>,
        phase: u8,
    },
    Sink {
        maximum_bytes: u32,
        pending: bool,
        next_request: u32,
    },
}

impl Operation for PortableStateInputOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source {
                value,
                emitted: false,
            } => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
            Self::Source { .. } => OperationAction::Complete,
            Self::Count { values, .. } | Self::Toggle { values, .. } => OperationAction::Emit {
                port: PortId(0),
                value: values[0],
            },
            Self::KeyTee { .. } | Self::Sink { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Count {
                    values,
                    next,
                    initial_emitted,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if *initial_emitted
                && value.byte_len == conduit_time::TICK_ENCODED_LEN
                && *next == 0 =>
            {
                *next = 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value: values[1],
                }
            }
            (
                Self::Toggle {
                    values,
                    next,
                    initial_emitted,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if *initial_emitted
                && value.byte_len == conduit_time::TICK_ENCODED_LEN
                && *next == 0 =>
            {
                *next = 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value: values[1],
                }
            }
            (
                Self::Count {
                    initial_emitted: true,
                    ..
                }
                | Self::Toggle {
                    initial_emitted: true,
                    ..
                },
                OperationInput::Closed { port: PortId(0) },
            ) => OperationAction::Complete,
            (
                Self::KeyTee { pending, phase },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if pending.is_none()
                && conduit_semantic_catalog::key_event_tee_accepts_encoded_len(value.byte_len) =>
            {
                *pending = Some(value);
                *phase = 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            (Self::KeyTee { pending: None, .. }, OperationInput::Closed { port: PortId(0) }) => {
                OperationAction::Complete
            }
            (
                Self::Sink {
                    maximum_bytes,
                    pending,
                    next_request,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*pending => {
                let Ok(input) = BoundedValueRef::new(value, *maximum_bytes) else {
                    return invalid(61);
                };
                let request = RequestId(*next_request);
                *next_request = next_request.saturating_add(1);
                *pending = true;
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            (
                Self::Sink { pending, .. },
                OperationInput::HostOperationCompleted { outcome, .. },
            ) if *pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = false;
                OperationAction::Await
            }
            (Self::Sink { pending: false, .. }, OperationInput::Closed { port: PortId(0) }) => {
                OperationAction::Complete
            }
            _ => invalid(60),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { emitted, .. } => {
                *emitted = true;
                OperationAction::Complete
            }
            Self::Count {
                initial_emitted, ..
            }
            | Self::Toggle {
                initial_emitted, ..
            } => {
                *initial_emitted = true;
                OperationAction::Await
            }
            Self::KeyTee {
                pending: Some(value),
                phase: 1,
            } => {
                let value = *value;
                *self = Self::KeyTee {
                    pending: Some(value),
                    phase: 2,
                };
                OperationAction::Emit {
                    port: PortId(1),
                    value,
                }
            }
            Self::KeyTee {
                pending: Some(_),
                phase: 2,
            } => {
                *self = Self::KeyTee {
                    pending: None,
                    phase: 0,
                };
                OperationAction::Await
            }
            _ => OperationAction::Await,
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::KeyTee { pending, phase } => {
                *pending = None;
                *phase = 0;
            }
            Self::Sink { pending, .. } => *pending = false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{Failure, FailureCode};

    fn value(slot: u16, byte_len: u32) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len,
        }
    }

    #[test]
    fn key_tee_refuses_wrong_shape_and_pressure_without_partial_second_branch() {
        let mut tee = PortableStateInputOperation::KeyTee {
            pending: None,
            phase: 0,
        };
        assert_eq!(tee.start(), OperationAction::Await);
        assert_eq!(
            tee.resume(OperationInput::Value {
                port: PortId(0),
                value: value(1, 1)
            }),
            OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 60
            })
        );
        let exact = value(2, conduit_human::KEY_EVENT_ENCODED_LEN as u32);
        assert_eq!(
            tee.resume(OperationInput::Value {
                port: PortId(0),
                value: exact
            }),
            OperationAction::Emit {
                port: PortId(0),
                value: exact
            }
        );
        assert!(matches!(
            tee.resume(OperationInput::Value {
                port: PortId(0),
                value: exact
            }),
            OperationAction::Fail(_)
        ));
        assert_eq!(
            tee.advance(),
            OperationAction::Emit {
                port: PortId(1),
                value: exact
            }
        );
        assert_eq!(tee.advance(), OperationAction::Await);
    }

    #[test]
    fn cancellation_releases_pending_tee_and_sink_state() {
        let exact = value(2, conduit_human::KEY_EVENT_ENCODED_LEN as u32);
        let mut tee = PortableStateInputOperation::KeyTee {
            pending: Some(exact),
            phase: 1,
        };
        tee.cancel();
        assert_eq!(tee.advance(), OperationAction::Await);
        assert_eq!(
            tee.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Complete
        );

        let mut sink = PortableStateInputOperation::Sink {
            maximum_bytes: 8,
            pending: true,
            next_request: 1,
        };
        sink.cancel();
        assert_eq!(
            sink.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Complete
        );
    }
}
