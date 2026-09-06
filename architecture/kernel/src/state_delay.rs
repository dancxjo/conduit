//! Allocation-independent explicit state/delay transition primitive.

pub mod operation;
pub mod transfer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateValueIdentity {
    pub slot: u16,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateTransition {
    Initialized,
    CandidateAccepted,
    Committed,
    HeldWithoutCandidate,
    Reset,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateEvidence {
    pub state: u16,
    pub generation: u64,
    pub current: StateValueIdentity,
    pub candidate: Option<StateValueIdentity>,
    pub transition: StateTransition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateError {
    InvalidBounds,
    ValueTooLarge,
    MultipleCandidates,
    TransitionLimitReached,
    IdentityCapacityExhausted,
}

/// One admitted state cell. `BYTES` is the fixed-storage ceiling; hosted use
/// selects the same machine with a larger pre-Play constant.
pub struct StateDelay<const BYTES: usize> {
    state: u16,
    maximum_bytes: usize,
    maximum_transitions: Option<u64>,
    generation: u64,
    current_len: usize,
    candidate_len: Option<usize>,
    current: [u8; BYTES],
    candidate: [u8; BYTES],
}

impl<const BYTES: usize> StateDelay<BYTES> {
    pub fn new(
        state: u16,
        maximum_bytes: usize,
        maximum_transitions: u64,
        initial: &[u8],
    ) -> Result<Self, StateError> {
        Self::with_transition_budget(state, maximum_bytes, Some(maximum_transitions), initial)
    }

    /// Input-driven lifetime with no predeclared semantic transition count.
    /// Storage and generation identity remain finite; generation exhaustion
    /// refuses explicitly and does not reset or renew the state.
    pub fn externally_continued(
        state: u16,
        maximum_bytes: usize,
        initial: &[u8],
    ) -> Result<Self, StateError> {
        Self::with_transition_budget(state, maximum_bytes, None, initial)
    }

    fn with_transition_budget(
        state: u16,
        maximum_bytes: usize,
        maximum_transitions: Option<u64>,
        initial: &[u8],
    ) -> Result<Self, StateError> {
        if maximum_bytes == 0 || maximum_bytes > BYTES || maximum_transitions == Some(0) {
            return Err(StateError::InvalidBounds);
        }
        if initial.len() > maximum_bytes {
            return Err(StateError::ValueTooLarge);
        }
        let mut current = [0; BYTES];
        current[..initial.len()].copy_from_slice(initial);
        Ok(Self {
            state,
            maximum_bytes,
            maximum_transitions,
            generation: 0,
            current_len: initial.len(),
            candidate_len: None,
            current,
            candidate: [0; BYTES],
        })
    }

    pub fn current(&self) -> &[u8] {
        &self.current[..self.current_len]
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn initial_evidence(&self) -> StateEvidence {
        self.evidence(StateTransition::Initialized, None)
    }

    fn evidence(
        &self,
        transition: StateTransition,
        candidate: Option<StateValueIdentity>,
    ) -> StateEvidence {
        StateEvidence {
            state: self.state,
            generation: self.generation,
            current: StateValueIdentity {
                slot: 0,
                generation: self.generation,
            },
            candidate,
            transition,
        }
    }

    pub fn offer_next(&mut self, value: &[u8]) -> Result<StateEvidence, StateError> {
        if value.len() > self.maximum_bytes {
            return Err(StateError::ValueTooLarge);
        }
        if self.candidate_len.is_some() {
            return Err(StateError::MultipleCandidates);
        }
        let candidate = StateValueIdentity {
            slot: 1,
            generation: self
                .generation
                .checked_add(1)
                .ok_or(StateError::IdentityCapacityExhausted)?,
        };
        self.candidate[..value.len()].copy_from_slice(value);
        self.candidate_len = Some(value.len());
        Ok(self.evidence(StateTransition::CandidateAccepted, Some(candidate)))
    }

    /// The admitted transition point. Absence deterministically retains the
    /// current value. Failed or cancelled work must call `abort_step` instead.
    pub fn commit(&mut self) -> Result<StateEvidence, StateError> {
        if self
            .maximum_transitions
            .is_some_and(|maximum| self.generation >= maximum)
        {
            return Err(StateError::TransitionLimitReached);
        }
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or(StateError::IdentityCapacityExhausted)?;
        let candidate = self.candidate_len.map(|_| StateValueIdentity {
            slot: 1,
            generation: next_generation,
        });
        let transition = if let Some(len) = self.candidate_len.take() {
            self.current[..len].copy_from_slice(&self.candidate[..len]);
            self.current_len = len;
            StateTransition::Committed
        } else {
            StateTransition::HeldWithoutCandidate
        };
        self.generation = next_generation;
        Ok(self.evidence(transition, candidate))
    }

    pub fn abort_step(&mut self, cancelled: bool) -> StateEvidence {
        // offer_next only admits a candidate when its exact next identity fits.
        let candidate = self
            .candidate_len
            .and_then(|_| self.generation.checked_add(1))
            .map(|generation| StateValueIdentity {
                slot: 1,
                generation,
            });
        self.candidate_len = None;
        self.evidence(
            if cancelled {
                StateTransition::Cancelled
            } else {
                StateTransition::Failed
            },
            candidate,
        )
    }

    pub fn reset(&mut self, initial: &[u8]) -> Result<StateEvidence, StateError> {
        if initial.len() > self.maximum_bytes {
            return Err(StateError::ValueTooLarge);
        }
        self.current[..initial.len()].copy_from_slice(initial);
        self.current_len = initial.len();
        self.candidate_len = None;
        self.generation = 0;
        Ok(self.evidence(StateTransition::Reset, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_bytes(value: u32) -> [u8; 4] {
        value.to_le_bytes()
    }

    fn run_recurrence<const BYTES: usize>(initial: u32, deltas: &[u32]) -> u32 {
        let mut state =
            StateDelay::<BYTES>::new(7, 4, deltas.len() as u64, &u32_bytes(initial)).unwrap();
        for delta in deltas {
            let current = u32::from_le_bytes(state.current().try_into().unwrap());
            state.offer_next(&u32_bytes(current + delta)).unwrap();
            assert_eq!(
                state.commit().unwrap().transition,
                StateTransition::Committed
            );
        }
        u32::from_le_bytes(state.current().try_into().unwrap())
    }

    #[test]
    fn fixed_and_hosted_profiles_share_numeric_recurrence_semantics() {
        let fixed = run_recurrence::<4>(0, &[1, 1, 1]);
        let hosted = run_recurrence::<4096>(0, &[1, 1, 1]);
        assert_eq!(fixed, hosted);
        assert_eq!(fixed, 3);
    }

    #[test]
    fn same_primitive_expresses_parameter_updates() {
        let parameter = run_recurrence::<4>(100, &[2, 3, 5]);
        assert_eq!(parameter, 110);
    }

    #[test]
    fn absent_and_multiple_candidates_have_finite_outcomes() {
        let mut state = StateDelay::<8>::new(1, 8, 2, b"old").unwrap();
        assert_eq!(
            state.commit().unwrap().transition,
            StateTransition::HeldWithoutCandidate
        );
        assert_eq!(state.current(), b"old");
        state.offer_next(b"new").unwrap();
        assert_eq!(
            state.offer_next(b"other"),
            Err(StateError::MultipleCandidates)
        );
        assert_eq!(
            state.commit().unwrap().transition,
            StateTransition::Committed
        );
        assert_eq!(state.current(), b"new");
        assert_eq!(state.commit(), Err(StateError::TransitionLimitReached));
    }

    #[test]
    fn cancellation_failure_and_reset_are_deterministic_and_observable() {
        let mut state = StateDelay::<8>::new(1, 8, 3, b"initial").unwrap();
        assert_eq!(
            state.initial_evidence().transition,
            StateTransition::Initialized
        );
        assert_eq!(state.initial_evidence().generation, 0);
        assert_eq!(state.current(), b"initial");
        state.offer_next(b"cancel").unwrap();
        let cancelled = state.abort_step(true);
        assert_eq!(cancelled.transition, StateTransition::Cancelled);
        assert!(cancelled.candidate.is_some());
        assert_eq!(state.current(), b"initial");
        state.offer_next(b"failure").unwrap();
        assert_eq!(state.abort_step(false).transition, StateTransition::Failed);
        assert_eq!(state.current(), b"initial");
        let reset = state.reset(b"reset").unwrap();
        assert_eq!(reset.transition, StateTransition::Reset);
        assert_eq!(reset.generation, 0);
        assert_eq!(state.current(), b"reset");
    }
}

#[cfg(test)]
#[path = "state_delay/continuation_tests.rs"]
mod continuation_tests;
