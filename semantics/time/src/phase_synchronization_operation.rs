//! Shared deterministic kernel operation for finite phase following.

use conduit_kernel::{
    CanonicalValue, Failure, FailureCode, Operation, OperationAction, OperationInput, PortId,
    ValueRef,
};

use crate::{
    decode_pulse_observation, decode_rhythm_state, encode_rhythm_state, synchronize,
    PulseObservation, RhythmState, SynchronizationOutcome,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Prepared,
    Ready,
    Emitting,
    Terminal,
    Cancelled,
}

/// Pairs one local state with one peer observation and derives one exact state.
pub struct PhaseSynchronizationOperation {
    local: Option<RhythmState>,
    peer: Option<PulseObservation>,
    closed: [bool; 2],
    last_outcome: Option<SynchronizationOutcome>,
    lifecycle: Lifecycle,
}

impl PhaseSynchronizationOperation {
    pub const fn new() -> Self {
        Self {
            local: None,
            peer: None,
            closed: [false; 2],
            last_outcome: None,
            lifecycle: Lifecycle::Prepared,
        }
    }

    pub const fn last_outcome(&self) -> Option<SynchronizationOutcome> {
        self.last_outcome
    }

    fn derive_if_ready(&mut self) -> OperationAction {
        let (Some(mut local), Some(peer)) = (self.local, self.peer) else {
            return OperationAction::Await;
        };
        let observed_at_ms = peer.sequence.wrapping_mul(u32::from(peer.period_ms));
        let outcome = match synchronize(&mut local, peer, observed_at_ms) {
            Ok(outcome) => outcome,
            Err(_) => return failure(FailureCode::InvalidInput, 511),
        };
        let value = match CanonicalValue::new(&encode_rhythm_state(local)) {
            Ok(value) => value,
            Err(_) => return failure(FailureCode::StorageExhausted, 512),
        };
        self.last_outcome = Some(outcome);
        self.lifecycle = Lifecycle::Emitting;
        OperationAction::EmitCanonical {
            port: PortId(0),
            value,
        }
    }
}

impl Default for PhaseSynchronizationOperation {
    fn default() -> Self {
        Self::new()
    }
}

impl Operation for PhaseSynchronizationOperation {
    fn start(&mut self) -> OperationAction {
        if self.lifecycle != Lifecycle::Prepared {
            return failure(FailureCode::InvalidLifecycle, 500);
        }
        self.lifecycle = Lifecycle::Ready;
        OperationAction::Await
    }

    fn resume_value(&mut self, port: PortId, _: ValueRef, canonical: &[u8]) -> OperationAction {
        if self.lifecycle == Lifecycle::Cancelled {
            return failure(FailureCode::Cancelled, 509);
        }
        if self.lifecycle != Lifecycle::Ready {
            return failure(FailureCode::InvalidLifecycle, 501);
        }
        match port {
            PortId(0) if self.local.is_none() && !self.closed[0] => {
                self.local = match decode_rhythm_state(canonical) {
                    Ok(value) => Some(value),
                    Err(_) => return failure(FailureCode::InvalidInput, 502),
                };
            }
            PortId(1) if self.peer.is_none() && !self.closed[1] => {
                self.peer = match decode_pulse_observation(canonical) {
                    Ok(value) => Some(value),
                    Err(_) => return failure(FailureCode::InvalidInput, 503),
                };
            }
            PortId(0) | PortId(1) => return failure(FailureCode::InvalidLifecycle, 504),
            _ => return failure(FailureCode::InvalidPort, 505),
        }
        self.derive_if_ready()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        if self.lifecycle == Lifecycle::Cancelled {
            return failure(FailureCode::Cancelled, 509);
        }
        if self.lifecycle != Lifecycle::Ready {
            return failure(FailureCode::InvalidLifecycle, 506);
        }
        let OperationInput::Closed { port } = input else {
            return failure(FailureCode::InvalidLifecycle, 507);
        };
        let Some(closed) = self.closed.get_mut(usize::from(port.0)) else {
            return failure(FailureCode::InvalidPort, 505);
        };
        *closed = true;
        if self.closed == [true, true] {
            if self.local.is_some() || self.peer.is_some() {
                return failure(FailureCode::InvalidInput, 508);
            }
            self.lifecycle = Lifecycle::Terminal;
            return OperationAction::Complete;
        }
        OperationAction::Await
    }

    fn advance(&mut self) -> OperationAction {
        if self.lifecycle == Lifecycle::Cancelled {
            return failure(FailureCode::Cancelled, 509);
        }
        if self.lifecycle != Lifecycle::Emitting {
            return failure(FailureCode::InvalidLifecycle, 510);
        }
        self.local = None;
        self.peer = None;
        self.lifecycle = Lifecycle::Ready;
        OperationAction::Await
    }

    fn cancel(&mut self) {
        self.lifecycle = Lifecycle::Cancelled;
    }
}

fn failure(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(len: usize) -> ValueRef {
        ValueRef {
            slot: 0,
            generation: 1,
            byte_len: len as u32,
        }
    }

    fn local(expected: u32) -> RhythmState {
        RhythmState {
            sequence: 2,
            next_pulse_at_ms: 960,
            period_ms: 240,
            expected_peer_sequence: expected,
        }
    }

    fn pair(
        operation: &mut PhaseSynchronizationOperation,
        state: RhythmState,
        peer: PulseObservation,
    ) -> OperationAction {
        assert_eq!(
            operation.resume_value(
                PortId(0),
                value(crate::RHYTHM_STATE_ENCODED_LEN),
                &encode_rhythm_state(state),
            ),
            OperationAction::Await
        );
        operation.resume_value(
            PortId(1),
            value(crate::PULSE_OBSERVATION_ENCODED_LEN),
            &crate::encode_pulse_observation(peer),
        )
    }

    #[test]
    fn stale_outside_and_adjusted_outcomes_remain_distinct() {
        let mut operation = PhaseSynchronizationOperation::new();
        operation.start();
        assert!(matches!(
            pair(
                &mut operation,
                local(4),
                PulseObservation {
                    sequence: 3,
                    period_ms: 240,
                },
            ),
            OperationAction::EmitCanonical { .. }
        ));
        assert_eq!(
            operation.last_outcome(),
            Some(SynchronizationOutcome::Stale)
        );
        operation.advance();
        assert!(matches!(
            pair(
                &mut operation,
                local(8),
                PulseObservation {
                    sequence: 8,
                    period_ms: 240,
                },
            ),
            OperationAction::EmitCanonical { .. }
        ));
        assert_eq!(
            operation.last_outcome(),
            Some(SynchronizationOutcome::OutsideWindow)
        );
    }

    #[test]
    fn malformed_missing_duplicate_and_cancel_are_machine_distinct() {
        let mut malformed = PhaseSynchronizationOperation::new();
        malformed.start();
        assert_eq!(
            malformed.resume_value(PortId(0), value(1), &[0]),
            failure(FailureCode::InvalidInput, 502)
        );

        let mut missing = PhaseSynchronizationOperation::new();
        missing.start();
        assert_eq!(
            missing.resume_value(
                PortId(0),
                value(crate::RHYTHM_STATE_ENCODED_LEN),
                &encode_rhythm_state(local(0)),
            ),
            OperationAction::Await
        );
        assert_eq!(
            missing.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Await
        );
        assert_eq!(
            missing.resume(OperationInput::Closed { port: PortId(1) }),
            failure(FailureCode::InvalidInput, 508)
        );

        let mut cancelled = PhaseSynchronizationOperation::new();
        cancelled.start();
        cancelled.cancel();
        assert_eq!(
            cancelled.resume(OperationInput::Closed { port: PortId(0) }),
            failure(FailureCode::Cancelled, 509)
        );
    }
}
