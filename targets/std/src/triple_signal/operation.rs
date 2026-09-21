use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};
use conduit_signal::SIGNAL_ENCODED_LEN;

pub(super) enum TripleBack {
    Pulse {
        values: Vec<ValueRef>,
        waits: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
        emitted: bool,
    },
    Show {
        expected: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
}

impl TripleBack {
    pub(super) fn pulse(values: Vec<ValueRef>, waits: Vec<ValueRef>) -> Self {
        Self::Pulse {
            values,
            waits,
            next: 0,
            pending: None,
            emitted: false,
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

    fn fail(code: FailureCode, detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure { code, detail })
    }
}

impl<const PORTS: usize> StepOperation<PORTS> for TripleBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Pulse {
                values,
                waits,
                next,
                pending,
                emitted,
            } => {
                if let Some(expected) = *pending {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    let Some(value) = values.get(*next).copied() else {
                        return Self::fail(FailureCode::InvalidLifecycle, 1);
                    };
                    if request != expected
                        || outcome.disposition != HostCallDisposition::Completed
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                        || !io.output_ready(PortId(0))
                        || io.consume_host_completion().is_err()
                        || io.send(PortId(0), value).is_err()
                    {
                        return Self::fail(FailureCode::InvalidLifecycle, 3);
                    }
                    *pending = None;
                    *emitted = true;
                    return StepOutcome::Progress;
                }
                if !*emitted {
                    let Some(value) = values.get(*next).copied() else {
                        return StepOutcome::Complete;
                    };
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if io.send(PortId(0), value).is_err() {
                        return Self::fail(FailureCode::InvalidLifecycle, 3);
                    }
                    *emitted = true;
                    return StepOutcome::Progress;
                }
                *next += 1;
                if *next >= values.len() {
                    return StepOutcome::Complete;
                }
                let Some(wait) = waits.get(*next - 1).copied() else {
                    return Self::fail(FailureCode::InvalidLifecycle, 5);
                };
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(FailureCode::InvalidLifecycle, 2);
                };
                let request = RequestId(sequence);
                let input = BoundedValueRef::new(wait, 8).expect("wait is exactly eight bytes");
                if io.request_host_call(request, HostCallId(0), input).is_err() {
                    return Self::fail(FailureCode::InvalidLifecycle, 3);
                }
                *pending = Some(request);
                *emitted = false;
                StepOutcome::Progress
            }
            Self::Show {
                expected,
                next,
                pending,
            } => {
                if let Some(expected_request) = *pending {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    if request != expected_request
                        || outcome.disposition != HostCallDisposition::Completed
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                        || io.consume_host_completion().is_err()
                    {
                        return Self::fail(FailureCode::InvalidInput, 4);
                    }
                    *pending = None;
                    *next += 1;
                    return StepOutcome::Progress;
                }
                if let Some(value) = io.input(PortId(0)) {
                    if expected.get(*next) != Some(&value) {
                        return Self::fail(FailureCode::InvalidInput, 4);
                    }
                    let Ok(sequence) = u32::try_from(*next) else {
                        return Self::fail(FailureCode::InvalidLifecycle, 2);
                    };
                    let request = RequestId(0x8000_0000 | sequence);
                    let input = BoundedValueRef::new(value, SIGNAL_ENCODED_LEN)
                        .expect("sealed Signal is exactly admitted");
                    if io.consume(PortId(0)).is_err()
                        || io.request_host_call(request, HostCallId(0), input).is_err()
                    {
                        return Self::fail(FailureCode::InvalidInput, 4);
                    }
                    *pending = Some(request);
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    if *next != expected.len() || io.consume_closed(PortId(0)).is_err() {
                        return Self::fail(FailureCode::InvalidInput, 4);
                    }
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Pulse { pending, .. } | Self::Show { pending, .. } => *pending = None,
        }
    }
}
