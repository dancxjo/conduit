//! Exact-port adapter for explicit State in the ordinary operation scheduler.

use super::{StateDelay, StateError};
use crate::{
    CanonicalValue, Failure, FailureCode, Operation, OperationAction, OperationInput, PortId,
    ValueRef,
};

/// This profile uses the kernel's existing bounded derived-value envelope.
/// Larger profiles must refuse construction rather than truncate State.
pub struct StateOperation<const BYTES: usize> {
    state: StateDelay<BYTES>,
    next: PortId,
    current: PortId,
    started: bool,
    terminal: bool,
}

impl<const BYTES: usize> StateOperation<BYTES> {
    pub fn new(
        state: StateDelay<BYTES>,
        next: PortId,
        current: PortId,
    ) -> Result<Self, StateError> {
        if state.maximum_bytes > CanonicalValue::MAXIMUM_BYTES {
            return Err(StateError::InvalidBounds);
        }
        Ok(Self {
            state,
            next,
            current,
            started: false,
            terminal: false,
        })
    }

    /// Move retained ownership after the containing driver has been retired.
    /// No generation, continuation allowance or committed byte is reset.
    pub fn into_state(self) -> StateDelay<BYTES> {
        self.state
    }

    pub fn state(&self) -> &StateDelay<BYTES> {
        &self.state
    }

    fn emit_current(&self) -> OperationAction {
        match CanonicalValue::new(self.state.current()) {
            Ok(value) => OperationAction::EmitCanonical {
                port: self.current,
                value,
            },
            Err(_) => OperationAction::Fail(Failure {
                code: FailureCode::StorageExhausted,
                detail: 4,
            }),
        }
    }

    fn refuse(&mut self, code: FailureCode, detail: u16) -> OperationAction {
        self.state.abort_step(false);
        self.terminal = true;
        OperationAction::Fail(Failure { code, detail })
    }

    fn state_refusal(&mut self, error: StateError) -> OperationAction {
        // Stable profile details retain exhaustion causes through OperationFailed.
        let (code, detail) = match error {
            StateError::ValueTooLarge => (FailureCode::StorageExhausted, 1),
            StateError::TransitionLimitReached => (FailureCode::StorageExhausted, 2),
            StateError::IdentityCapacityExhausted => (FailureCode::StorageExhausted, 3),
            StateError::InvalidBounds => (FailureCode::InvalidInput, 5),
            StateError::MultipleCandidates => (FailureCode::InvalidLifecycle, 6),
        };
        self.refuse(code, detail)
    }
}

impl<const BYTES: usize> Operation for StateOperation<BYTES> {
    fn start(&mut self) -> OperationAction {
        if self.started || self.terminal {
            return self.refuse(FailureCode::InvalidLifecycle, 7);
        }
        self.started = true;
        self.emit_current()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        if !self.started || self.terminal {
            return self.refuse(FailureCode::InvalidLifecycle, 7);
        }
        match input {
            OperationInput::Closed { port } if port == self.next => {
                self.terminal = true;
                OperationAction::Complete
            }
            _ => self.refuse(FailureCode::InvalidInput, 8),
        }
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        if !self.started || self.terminal {
            return self.refuse(FailureCode::InvalidLifecycle, 7);
        }
        if port != self.next || canonical.len() != value.byte_len as usize {
            return self.refuse(FailureCode::InvalidInput, 8);
        }
        if self
            .state
            .maximum_transitions
            .is_some_and(|maximum| self.state.generation >= maximum)
        {
            return self.state_refusal(StateError::TransitionLimitReached);
        }
        if let Err(error) = self.state.offer_next(canonical) {
            return self.state_refusal(error);
        }
        OperationAction::EmitCanonical {
            port: self.current,
            value: CanonicalValue::new(canonical)
                .expect("admitted State fits the canonical envelope"),
        }
    }

    fn step_committed(&mut self) {
        if self.state.candidate_len.is_some() {
            self.state
                .commit()
                .expect("candidate transition and identity bounds were admitted before I/O");
        }
    }

    fn cancel(&mut self) {
        self.state.abort_step(true);
        self.terminal = true;
    }
}
