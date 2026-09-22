//! Step Backs for the distributed toggle source fragment.

use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CapacitySeal {
    pub values: (usize, usize),
    pub sign: usize,
    pub backs: usize,
    pub identity: (usize, usize, usize),
}

/// The two finite Backs in the source fragment: an attended trigger and a
/// stateful Boolean toggle.
pub(super) enum ToggleSourceBack {
    Trigger {
        tokens: Vec<ValueRef>,
        values: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
    Toggle {
        values: Vec<ValueRef>,
        expected_triggers: Vec<ValueRef>,
        next: usize,
        initial_emitted: bool,
    },
}

impl ToggleSourceBack {
    fn fail(detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        match self {
            Self::Trigger { tokens, values, .. } => tokens.capacity() + values.capacity(),
            Self::Toggle {
                values,
                expected_triggers,
                ..
            } => values.capacity() + expected_triggers.capacity(),
        }
    }
}

impl<const PORTS: usize> StepBack<PORTS> for ToggleSourceBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Trigger {
                tokens,
                values,
                next,
                pending,
            } => {
                if let Some(expected) = *pending {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    let Some(value) = values.get(*next).copied() else {
                        return Self::fail(11);
                    };
                    if request != expected
                        || outcome.disposition != HostCallDisposition::Completed
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                        || !io.output_ready(PortId(0))
                        || io.consume_host_completion().is_err()
                        || io.send(PortId(0), value).is_err()
                    {
                        return Self::fail(13);
                    }
                    *pending = None;
                    *next += 1;
                    return StepOutcome::Progress;
                }

                let Some(token) = tokens.get(*next).copied() else {
                    return if *next == values.len() {
                        StepOutcome::Complete
                    } else {
                        Self::fail(14)
                    };
                };
                if *next >= values.len() {
                    return Self::fail(11);
                }
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(15);
                };
                let request = RequestId(sequence);
                let input = BoundedValueRef::new(token, 1)
                    .expect("sealed trigger token is exactly admitted");
                if io.request_host_call(request, HostCallId(0), input).is_err() {
                    return Self::fail(13);
                }
                *pending = Some(request);
                StepOutcome::Progress
            }
            Self::Toggle {
                values,
                expected_triggers,
                next,
                initial_emitted,
            } => {
                if !*initial_emitted {
                    let Some(value) = values.first().copied() else {
                        return Self::fail(12);
                    };
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if io.send(PortId(0), value).is_err() {
                        return Self::fail(13);
                    }
                    *initial_emitted = true;
                    return StepOutcome::Progress;
                }

                if let Some(value) = io.input(PortId(0)) {
                    let Some(expected) = expected_triggers.get(*next).copied() else {
                        return Self::fail(16);
                    };
                    if value != expected {
                        return Self::fail(17);
                    }
                    let Some(output) = values.get(*next + 1).copied() else {
                        return Self::fail(12);
                    };
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if io.consume(PortId(0)).is_err() || io.send(PortId(0), output).is_err() {
                        return Self::fail(13);
                    }
                    *next += 1;
                    return StepOutcome::Progress;
                }

                if io.input_closed(PortId(0)) {
                    if *next != expected_triggers.len() || io.consume_closed(PortId(0)).is_err() {
                        return Self::fail(13);
                    }
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
        }
    }

    fn cancel(&mut self) {
        if let Self::Trigger { pending, .. } = self {
            *pending = None;
        }
    }
}
