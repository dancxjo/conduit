//! Shared kernel operation for finite trailing debounce.

use alloc::vec::Vec;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebouncePreparationError {
    Empty,
    InsufficientDurations,
    RequestIdentityOverflow,
}

/// One finite trailing-debounce operation over already admitted values.
///
/// The caller stores one exact eight-byte monotonic duration value for every
/// value the operation may accept. The operation retains at most one ordinary
/// input and owns no clock: each deadline crosses Host Call zero.
pub struct TrailingDebounceBack {
    durations: Vec<ValueRef>,
    next_request: usize,
    maximum_values: usize,
    accepted_values: usize,
    pending: Option<RequestId>,
    cancellation_requested: bool,
    candidate: Option<ValueRef>,
    closing: bool,
}

impl TrailingDebounceBack {
    pub fn from_prepared_durations(
        durations: Vec<ValueRef>,
        maximum_values: usize,
    ) -> Result<Self, DebouncePreparationError> {
        if maximum_values == 0 {
            return Err(DebouncePreparationError::Empty);
        }
        if durations.len() < maximum_values {
            return Err(DebouncePreparationError::InsufficientDurations);
        }
        if u32::try_from(maximum_values).is_err() {
            return Err(DebouncePreparationError::RequestIdentityOverflow);
        }
        Ok(Self {
            durations,
            next_request: 0,
            maximum_values,
            accepted_values: 0,
            pending: None,
            cancellation_requested: false,
            candidate: None,
            closing: false,
        })
    }

    fn arm<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> Result<(), StepOutcome> {
        let Some(duration) = self.durations.get(self.next_request).copied() else {
            return Err(fail(51));
        };
        let request = RequestId(u32::try_from(self.next_request + 1).map_err(|_| fail(51))?);
        io.request_host_call(
            request,
            HostCallId(0),
            BoundedValueRef::new(duration, 8).map_err(|_| fail(51))?,
        )
        .map_err(|_| fail(51))?;
        self.next_request += 1;
        self.pending = Some(request);
        self.cancellation_requested = false;
        Ok(())
    }

    fn observe_superseded_completion<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
    ) -> Result<(), StepOutcome> {
        let Some((request, outcome)) = io.host_completion() else {
            return Ok(());
        };
        if self.pending != Some(request)
            || outcome.output.is_some()
            || outcome.failure.is_some()
            || !matches!(
                outcome.disposition,
                HostCallDisposition::Completed | HostCallDisposition::Cancelled
            )
        {
            return Err(outcome.failure.map_or_else(|| fail(53), StepOutcome::Fail));
        }
        io.consume_host_completion().map_err(|_| fail(53))?;
        self.pending = None;
        self.cancellation_requested = false;
        Ok(())
    }

    fn discard_one_unused<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if self.durations.len() > self.next_request {
            let value = self.durations.pop().expect("unused duration exists");
            if io.discard(value).is_err() {
                return fail(55);
            }
            StepOutcome::Progress
        } else {
            StepOutcome::Complete
        }
    }
}

impl<const PORTS: usize> StepBack<PORTS> for TrailingDebounceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        // Input wins a tie with the old deadline: the newly accepted value gets
        // one complete duration before it may be emitted.
        if let Some(value) = io.input(PortId(0)) {
            if self.closing {
                return fail(52);
            }
            if self.accepted_values >= self.maximum_values {
                return failure(FailureCode::WorkBudgetExhausted, 52);
            }
            let retained = match io.take_input(PortId(0)) {
                Ok(value) => value,
                Err(_) => return fail(52),
            };
            debug_assert_eq!(retained, value);
            if let Some(previous) = self.candidate.replace(retained) {
                if io.discard(previous).is_err() {
                    return fail(52);
                }
            }
            self.accepted_values += 1;

            if io.host_completion().is_some() {
                if let Err(outcome) = self.observe_superseded_completion(io) {
                    return outcome;
                }
                if let Err(outcome) = self.arm(io) {
                    return outcome;
                }
            } else if let Some(request) = self.pending {
                if !self.cancellation_requested {
                    if io.cancel_host_call(request).is_err() {
                        return fail(53);
                    }
                    self.cancellation_requested = true;
                }
            } else if let Err(outcome) = self.arm(io) {
                return outcome;
            }
            return StepOutcome::Progress;
        }

        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return outcome.failure.map_or_else(|| fail(53), StepOutcome::Fail);
            }

            // A completion which raced an already requested cancellation is
            // still the superseded deadline. It never releases the replacement
            // value early.
            if self.cancellation_requested {
                if !matches!(
                    outcome.disposition,
                    HostCallDisposition::Completed | HostCallDisposition::Cancelled
                ) || io.consume_host_completion().is_err()
                {
                    return fail(53);
                }
                self.pending = None;
                self.cancellation_requested = false;
                if self.closing {
                    if let Some(value) = self.candidate.take() {
                        if !io.output_ready(PortId(0)) {
                            self.candidate = Some(value);
                            return StepOutcome::Await;
                        }
                        if io.send(PortId(0), value).is_err() {
                            return fail(53);
                        }
                    }
                    return self.discard_one_unused(io);
                }
                return match self.arm(io) {
                    Ok(()) => StepOutcome::Progress,
                    Err(outcome) => outcome,
                };
            }

            match outcome.disposition {
                HostCallDisposition::Completed => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let Some(value) = self.candidate.take() else {
                        return fail(54);
                    };
                    if io.consume_host_completion().is_err() || io.send(PortId(0), value).is_err() {
                        return fail(53);
                    }
                    self.pending = None;
                    if self.closing {
                        self.discard_one_unused(io)
                    } else {
                        StepOutcome::Progress
                    }
                }
                HostCallDisposition::Cancelled => fail(53),
                _ => fail(53),
            }
        } else if io.input_closed(PortId(0)) && !self.closing {
            if io.consume_closed(PortId(0)).is_err() {
                return fail(53);
            }
            self.closing = true;
            if let Some(request) = self.pending {
                if !self.cancellation_requested {
                    if io.cancel_host_call(request).is_err() {
                        return fail(53);
                    }
                    self.cancellation_requested = true;
                }
                StepOutcome::Progress
            } else {
                if let Some(value) = self.candidate.take() {
                    if !io.output_ready(PortId(0)) {
                        self.candidate = Some(value);
                        return StepOutcome::Await;
                    }
                    if io.send(PortId(0), value).is_err() {
                        return fail(53);
                    }
                }
                self.discard_one_unused(io)
            }
        } else if self.closing && self.pending.is_none() {
            if let Some(value) = self.candidate.take() {
                if !io.output_ready(PortId(0)) {
                    self.candidate = Some(value);
                    return StepOutcome::Await;
                }
                if io.send(PortId(0), value).is_err() {
                    return fail(53);
                }
            }
            self.discard_one_unused(io)
        } else {
            StepOutcome::Await
        }
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        true
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.cancellation_requested = false;
        self.candidate = None;
    }
}

const fn fail(detail: u16) -> StepOutcome {
    failure(FailureCode::InvalidLifecycle, detail)
}

const fn failure(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests;
