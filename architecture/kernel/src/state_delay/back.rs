//! Exact-port Back for explicit State in the bounded Step scheduler.

use super::{StateDelay, StateError};
use crate::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use crate::{CanonicalValue, Failure, FailureCode, PortId};

/// Inline and explicitly prepared output profiles share the same state machine.
pub struct StateBack<const BYTES: usize> {
    state: StateDelay<BYTES>,
    next: PortId,
    current: PortId,
    started: bool,
    terminal: bool,
    prepared_outputs: bool,
    prepared_staged: bool,
}

impl<const BYTES: usize, const PORTS: usize> StepBack<PORTS> for StateBack<BYTES> {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.terminal {
            return self.step_refusal(FailureCode::InvalidLifecycle, 7);
        }
        if !self.started {
            if !io.output_ready(self.current) {
                return StepOutcome::Await;
            }
            if self.prepared_outputs {
                if io
                    .send_prepared(self.current, self.state.current_len as u32)
                    .is_err()
                {
                    return self.step_refusal(FailureCode::StateCapacityExhausted, 4);
                }
                self.prepared_staged = true;
            } else {
                let Ok(value) = CanonicalValue::new(self.state.current()) else {
                    return self.step_refusal(FailureCode::StorageExhausted, 4);
                };
                if io.send_canonical(self.current, value).is_err() {
                    return StepOutcome::Await;
                }
            }
            self.started = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(self.next) {
            if !io.output_ready(self.current) {
                return StepOutcome::Await;
            }
            let Some(canonical) = input_bytes.input(self.next) else {
                return self.step_refusal(FailureCode::InvalidInput, 8);
            };
            if canonical.len() != value.byte_len as usize {
                return self.step_refusal(FailureCode::InvalidInput, 8);
            }
            if self
                .state
                .maximum_transitions
                .is_some_and(|maximum| self.state.generation >= maximum)
            {
                return self.step_state_refusal(StateError::TransitionLimitReached);
            }
            if let Err(error) = self.state.offer_next(canonical) {
                return self.step_state_refusal(error);
            }
            io.consume(self.next).expect("present State input");
            if self.prepared_outputs {
                if io
                    .send_prepared(self.current, canonical.len() as u32)
                    .is_err()
                {
                    return self.step_refusal(FailureCode::StateCapacityExhausted, 4);
                }
                self.prepared_staged = true;
            } else {
                io.send_canonical(
                    self.current,
                    CanonicalValue::new(canonical).expect("admitted State canonical envelope"),
                )
                .expect("ready State output");
            }
            return StepOutcome::Progress;
        }
        if io.input_closed(self.next) {
            io.consume_closed(self.next)
                .expect("observed State closure");
            self.terminal = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn step_committed(&mut self) {
        if self.state.candidate_len.is_some() {
            self.state
                .commit()
                .expect("State transition bounds were admitted before I/O");
        }
        self.prepared_staged = false;
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if port != self.current || !self.prepared_staged {
            return None;
        }
        Some(if let Some(len) = self.state.candidate_len {
            &self.state.candidate[..len]
        } else {
            self.state.current()
        })
    }

    fn cancel(&mut self) {
        self.state.abort_step(true);
        self.terminal = true;
        self.prepared_staged = false;
    }
}

impl<const BYTES: usize> StateBack<BYTES> {
    fn step_refusal(&mut self, code: FailureCode, detail: u16) -> StepOutcome {
        self.state.abort_step(false);
        self.terminal = true;
        self.prepared_staged = false;
        StepOutcome::Fail(Failure { code, detail })
    }

    fn step_state_refusal(&mut self, error: StateError) -> StepOutcome {
        let (code, detail) = match error {
            StateError::ValueTooLarge => (FailureCode::StateCapacityExhausted, 1),
            StateError::TransitionLimitReached => (FailureCode::WorkBudgetExhausted, 2),
            StateError::IdentityCapacityExhausted => (FailureCode::IdentityCapacityExhausted, 3),
            StateError::InvalidBounds => (FailureCode::InvalidInput, 5),
            StateError::MultipleCandidates => (FailureCode::InvalidLifecycle, 6),
        };
        self.step_refusal(code, detail)
    }

    pub fn new(
        state: StateDelay<BYTES>,
        next: PortId,
        current: PortId,
    ) -> Result<Self, StateError> {
        if state.maximum_bytes > CanonicalValue::MAXIMUM_BYTES {
            return Err(StateError::InvalidBounds);
        }
        Ok(Self::construct(state, next, current, false))
    }

    /// Admit fixed State storage through the kernel's prepared-output boundary.
    /// The selected host profile must separately admit the exact output and
    /// value-store capacities; this does not enlarge the inline kernel profile.
    pub fn new_prepared(
        state: StateDelay<BYTES>,
        next: PortId,
        current: PortId,
    ) -> Result<Self, StateError> {
        if u32::try_from(state.maximum_bytes).is_err() {
            return Err(StateError::InvalidBounds);
        }
        Ok(Self::construct(state, next, current, true))
    }

    fn construct(
        state: StateDelay<BYTES>,
        next: PortId,
        current: PortId,
        prepared_outputs: bool,
    ) -> Self {
        Self {
            state,
            next,
            current,
            started: false,
            terminal: false,
            prepared_outputs,
            prepared_staged: false,
        }
    }

    /// Move retained ownership after the containing driver has been retired.
    /// No generation, continuation allowance or committed byte is reset.
    pub fn into_state(self) -> StateDelay<BYTES> {
        self.state
    }

    /// Terminal State can leave an owned retired driver; quiescence is not terminal.
    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub fn state(&self) -> &StateDelay<BYTES> {
        &self.state
    }

    pub const fn next_port(&self) -> PortId {
        self.next
    }
}
