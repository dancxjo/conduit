//! Fixed-storage operation for the portable current Scalar selector.

use conduit_core::{BOOL_ENCODED_LEN, InfoBool, SCALAR_ENCODED_LEN, Scalar};
use conduit_kernel::{
    CanonicalValue, Operation, OperationAction, OperationInput, PortId, ValueRef,
};

pub(super) enum StateSelectOperation {
    Source {
        values: [Option<ValueRef>; 2],
        phase: u8,
    },
    Select {
        selector: Option<bool>,
        candidates: [Option<[u8; SCALAR_ENCODED_LEN]>; 2],
        closed: [bool; 3],
    },
    Sink {
        pending: bool,
        next_request: u32,
    },
}

impl Operation for StateSelectOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { values, phase: 0 } => {
                values[0].map_or(OperationAction::Complete, |value| OperationAction::Emit {
                    port: PortId(0),
                    value,
                })
            }
            _ => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Source { values, phase },
                OperationInput::HostOperationCompleted {
                    request: conduit_kernel::RequestId(0),
                    outcome,
                },
            ) if *phase == 1
                && outcome.disposition == conduit_kernel::HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *phase = 2;
                OperationAction::Emit {
                    port: PortId(0),
                    value: values[1].expect("phase one exists only for a second value"),
                }
            }
            (Self::Select { closed, .. }, OperationInput::Closed { port })
                if usize::from(port.0) < closed.len() && !closed[usize::from(port.0)] =>
            {
                closed[usize::from(port.0)] = true;
                if closed.iter().all(|closed| *closed) {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            (
                Self::Sink {
                    pending,
                    next_request,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*pending && value.byte_len == SCALAR_ENCODED_LEN as u32 => {
                let request = conduit_kernel::RequestId(*next_request);
                *next_request = next_request.saturating_add(1);
                *pending = true;
                OperationAction::RequestHostOperation {
                    request,
                    operation: conduit_kernel::HostOperationId(0),
                    input: conduit_kernel::BoundedValueRef::new(value, SCALAR_ENCODED_LEN as u32)
                        .expect("exact Scalar length is within the sink bound"),
                }
            }
            (
                Self::Sink { pending, .. },
                OperationInput::HostOperationCompleted { outcome, .. },
            ) if *pending
                && outcome.disposition == conduit_kernel::HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = false;
                OperationAction::Await
            }
            (Self::Sink { pending: false, .. }, OperationInput::Closed { port: PortId(0) }) => {
                OperationAction::Complete
            }
            _ => invalid(71),
        }
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        let Self::Select {
            selector,
            candidates,
            closed,
        } = self
        else {
            return self.resume(OperationInput::Value { port, value });
        };
        match port {
            PortId(0) if value.byte_len == BOOL_ENCODED_LEN as u32 && !closed[0] => {
                let Ok(decoded) = InfoBool::decode(canonical) else {
                    return invalid(72);
                };
                *selector = Some(decoded.get());
            }
            PortId(1) | PortId(2)
                if value.byte_len == SCALAR_ENCODED_LEN as u32 && !closed[usize::from(port.0)] =>
            {
                if Scalar::decode(canonical).is_err() {
                    return invalid(72);
                }
                candidates[usize::from(port.0 - 1)] = Some(
                    canonical
                        .try_into()
                        .expect("decoded Scalar has exact canonical length"),
                );
            }
            _ => return invalid(72),
        }
        let Some(selected) = *selector else {
            return OperationAction::Await;
        };
        if candidates.iter().any(Option::is_none) {
            return OperationAction::Await;
        }
        let value = candidates[usize::from(selected)].expect("both candidates are present");
        OperationAction::EmitCanonical {
            port: PortId(0),
            value: CanonicalValue::new(&value).expect("Scalar fits the canonical value bound"),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source {
                values,
                phase: phase @ 0,
            } => match values[1] {
                Some(_) => {
                    *phase = 1;
                    OperationAction::RequestHostOperation {
                        request: conduit_kernel::RequestId(0),
                        operation: conduit_kernel::HostOperationId(0),
                        input: conduit_kernel::BoundedValueRef::new(
                            values[0].expect("a source always has its first value"),
                            SCALAR_ENCODED_LEN as u32,
                        )
                        .expect("source values fit the Scalar upper bound"),
                    }
                }
                None => OperationAction::Complete,
            },
            Self::Source { phase: 2, .. } => OperationAction::Complete,
            _ => OperationAction::Await,
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Select {
                selector,
                candidates,
                ..
            } => {
                *selector = None;
                *candidates = [None; 2];
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
