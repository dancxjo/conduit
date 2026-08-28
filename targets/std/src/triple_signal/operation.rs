use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};
use conduit_signal::SIGNAL_ENCODED_LEN;

pub(super) enum TripleOperation {
    Pulse {
        values: Vec<ValueRef>,
        waits: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
    Show {
        expected: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
}

impl TripleOperation {
    pub(super) fn pulse(values: Vec<ValueRef>, waits: Vec<ValueRef>) -> Self {
        Self::Pulse {
            values,
            waits,
            next: 0,
            pending: None,
        }
    }

    pub(super) fn show(expected: Vec<ValueRef>) -> Self {
        Self::Show {
            expected,
            next: 0,
            pending: None,
        }
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        match self {
            Self::Pulse { values, waits, .. } => values.capacity() + waits.capacity(),
            Self::Show { expected, .. } => expected.capacity(),
        }
    }

    fn fail(code: FailureCode, detail: u16) -> OperationAction {
        OperationAction::Fail(Failure { code, detail })
    }
}

impl Operation for TripleOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Pulse { values, .. } => {
                values
                    .first()
                    .copied()
                    .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    })
            }
            Self::Show { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Pulse {
                    values,
                    next,
                    pending,
                    ..
                },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                values.get(*next).copied().map_or_else(
                    || Self::fail(FailureCode::InvalidLifecycle, 1),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            (
                Self::Show {
                    expected,
                    next,
                    pending,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if pending.is_none() && expected.get(*next) == Some(&value) => {
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(FailureCode::InvalidLifecycle, 2);
                };
                let request = RequestId(0x8000_0000 | sequence);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, SIGNAL_ENCODED_LEN)
                        .expect("sealed Signal is exactly admitted"),
                }
            }
            (
                Self::Show { next, pending, .. },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                *next += 1;
                OperationAction::Await
            }
            (
                Self::Show {
                    expected,
                    next,
                    pending,
                },
                OperationInput::Closed { port: PortId(0) },
            ) if pending.is_none() && *next == expected.len() => OperationAction::Complete,
            (Self::Pulse { .. }, _) => Self::fail(FailureCode::InvalidLifecycle, 3),
            (Self::Show { .. }, _) => Self::fail(FailureCode::InvalidInput, 4),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Pulse {
                values,
                waits,
                next,
                pending,
            } => {
                *next += 1;
                if *next >= values.len() {
                    return OperationAction::Complete;
                }
                let Some(wait) = waits.get(*next - 1).copied() else {
                    return Self::fail(FailureCode::InvalidLifecycle, 5);
                };
                let request = RequestId(*next as u32);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(wait, 8).expect("wait is exactly eight bytes"),
                }
            }
            Self::Show { .. } => OperationAction::Await,
        }
    }
}
