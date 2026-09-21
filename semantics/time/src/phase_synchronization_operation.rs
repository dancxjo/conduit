//! Shared deterministic kernel operation for finite phase following.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, Failure, FailureCode, PortId,
};

use crate::{
    decode_pulse_observation, decode_rhythm_state, encode_rhythm_state, synchronize,
    PulseObservation, RhythmState, SynchronizationOutcome,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Ready,
    Terminal,
    Cancelled,
}

/// Retains one admitted local state while each peer observation derives an update.
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
            lifecycle: Lifecycle::Ready,
        }
    }

    pub const fn last_outcome(&self) -> Option<SynchronizationOutcome> {
        self.last_outcome
    }

    fn derive_if_ready<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        let (Some(local), Some(peer)) = (self.local.as_mut(), self.peer) else {
            return StepOutcome::Progress;
        };
        let observed_at_ms = peer.sequence.wrapping_mul(u32::from(peer.period_ms));
        let outcome = match synchronize(local, peer, observed_at_ms) {
            Ok(outcome) => outcome,
            Err(_) => return failure(FailureCode::InvalidInput, 511),
        };
        let value = match CanonicalValue::new(&encode_rhythm_state(*local)) {
            Ok(value) => value,
            Err(_) => return failure(FailureCode::StorageExhausted, 512),
        };
        self.last_outcome = Some(outcome);
        io.send_canonical(PortId(0), value)
            .expect("ready phase synchronization output");
        self.peer = None;
        StepOutcome::Progress
    }
}

impl Default for PhaseSynchronizationOperation {
    fn default() -> Self {
        Self::new()
    }
}

impl<const PORTS: usize> StepBack<PORTS> for PhaseSynchronizationOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.lifecycle == Lifecycle::Cancelled {
            return failure(FailureCode::Cancelled, 509);
        }
        if self.lifecycle != Lifecycle::Ready {
            return failure(FailureCode::InvalidLifecycle, 501);
        }
        let local_input = io.input(PortId(0)).is_some();
        let peer_input = io.input(PortId(1)).is_some();
        if (local_input && (self.local.is_some() || self.closed[0]))
            || (peer_input && (self.peer.is_some() || self.closed[1]))
        {
            return failure(FailureCode::InvalidLifecycle, 504);
        }
        if (self.local.is_some() || local_input)
            && (self.peer.is_some() || peer_input)
            && !io.output_ready(PortId(0))
        {
            return StepOutcome::Await;
        }
        if local_input {
            let Some(canonical) = input_bytes.input(PortId(0)) else {
                return failure(FailureCode::InvalidInput, 502);
            };
            self.local = match decode_rhythm_state(canonical) {
                Ok(value) => Some(value),
                Err(_) => return failure(FailureCode::InvalidInput, 502),
            };
            io.consume(PortId(0))
                .expect("present phase synchronization state");
        }
        if peer_input {
            let Some(canonical) = input_bytes.input(PortId(1)) else {
                return failure(FailureCode::InvalidInput, 503);
            };
            self.peer = match decode_pulse_observation(canonical) {
                Ok(value) => Some(value),
                Err(_) => return failure(FailureCode::InvalidInput, 503),
            };
            io.consume(PortId(1))
                .expect("present phase synchronization observation");
        }
        if local_input || peer_input {
            return self.derive_if_ready(io);
        }
        let mut observed_close = false;
        for port in [PortId(0), PortId(1)] {
            let index = usize::from(port.0);
            if io.input_closed(port) && !self.closed[index] {
                io.consume_closed(port)
                    .expect("observed phase synchronization closure");
                self.closed[index] = true;
                observed_close = true;
                break;
            }
        }
        if self.closed == [true, true] {
            if self.local.is_none() || self.peer.is_some() {
                return failure(FailureCode::InvalidInput, 508);
            }
            self.lifecycle = Lifecycle::Terminal;
            return StepOutcome::Complete;
        }
        if observed_close {
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }

    fn cancel(&mut self) {
        self.lifecycle = Lifecycle::Cancelled;
    }
}

fn failure(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(slot: u16, len: usize) -> conduit_kernel::ValueRef {
        conduit_kernel::ValueRef {
            slot,
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
    ) -> (StepOutcome, StepIo<2>) {
        let state_bytes = encode_rhythm_state(state);
        let mut state_io = StepIo::test_frame(
            [Some(value(0, state_bytes.len())), None],
            [false; 2],
            [Some(crate::RHYTHM_STATE_ENCODED_LEN as u32), None],
            None,
            4,
        );
        assert_eq!(
            operation.step(
                &mut state_io,
                &StepInputBytes::test_frame([Some(&state_bytes), None], None),
            ),
            StepOutcome::Progress
        );
        let peer_bytes = crate::encode_pulse_observation(peer);
        let mut peer_io = StepIo::test_frame(
            [None, Some(value(1, peer_bytes.len()))],
            [false; 2],
            [Some(crate::RHYTHM_STATE_ENCODED_LEN as u32), None],
            None,
            4,
        );
        let outcome = operation.step(
            &mut peer_io,
            &StepInputBytes::test_frame([None, Some(&peer_bytes)], None),
        );
        (outcome, peer_io)
    }

    #[test]
    fn stale_outside_and_adjusted_outcomes_remain_distinct() {
        let mut operation = PhaseSynchronizationOperation::new();
        assert_eq!(
            pair(
                &mut operation,
                local(4),
                PulseObservation {
                    sequence: 3,
                    period_ms: 240,
                },
            )
            .0,
            StepOutcome::Progress
        );
        assert_eq!(
            operation.last_outcome(),
            Some(SynchronizationOutcome::Stale)
        );
        let mut outside = PhaseSynchronizationOperation::new();
        assert_eq!(
            pair(
                &mut outside,
                local(8),
                PulseObservation {
                    sequence: 8,
                    period_ms: 240,
                },
            )
            .0,
            StepOutcome::Progress
        );
        assert_eq!(
            outside.last_outcome(),
            Some(SynchronizationOutcome::OutsideWindow)
        );
    }

    #[test]
    fn malformed_missing_duplicate_and_cancel_are_machine_distinct() {
        let mut malformed = PhaseSynchronizationOperation::new();
        let mut malformed_io = StepIo::test_frame(
            [Some(value(0, 1)), None],
            [false; 2],
            [Some(crate::RHYTHM_STATE_ENCODED_LEN as u32), None],
            None,
            4,
        );
        assert_eq!(
            malformed.step(
                &mut malformed_io,
                &StepInputBytes::test_frame([Some(&[0]), None], None),
            ),
            failure(FailureCode::InvalidInput, 502)
        );

        let mut missing = PhaseSynchronizationOperation::new();
        let local_bytes = encode_rhythm_state(local(0));
        let mut local_io = StepIo::test_frame(
            [Some(value(0, local_bytes.len())), None],
            [false; 2],
            [Some(crate::RHYTHM_STATE_ENCODED_LEN as u32), None],
            None,
            4,
        );
        assert_eq!(
            missing.step(
                &mut local_io,
                &StepInputBytes::test_frame([Some(&local_bytes), None], None),
            ),
            StepOutcome::Progress
        );
        let mut close_local = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 4);
        assert_eq!(
            missing.step(
                &mut close_local,
                &StepInputBytes::test_frame([None; 2], None)
            ),
            StepOutcome::Progress
        );
        let mut close_peer = StepIo::test_frame([None; 2], [false, true], [None; 2], None, 4);
        assert_eq!(
            missing.step(
                &mut close_peer,
                &StepInputBytes::test_frame([None; 2], None)
            ),
            StepOutcome::Complete
        );

        let mut no_local = PhaseSynchronizationOperation::new();
        let mut close_local = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 4);
        no_local.step(
            &mut close_local,
            &StepInputBytes::test_frame([None; 2], None),
        );
        let mut close_peer = StepIo::test_frame([None; 2], [false, true], [None; 2], None, 4);
        assert_eq!(
            no_local.step(
                &mut close_peer,
                &StepInputBytes::test_frame([None; 2], None)
            ),
            failure(FailureCode::InvalidInput, 508)
        );

        let mut cancelled = PhaseSynchronizationOperation::new();
        StepBack::<2>::cancel(&mut cancelled);
        let mut io = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 4);
        assert_eq!(
            cancelled.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            failure(FailureCode::Cancelled, 509)
        );
    }
}
