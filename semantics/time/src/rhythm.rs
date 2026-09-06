//! Portable, finite rhythm observation and phase-following semantics.

pub const PULSE_OBSERVATION_VALUE_KIND: &str = "time/pulse-observation@1";
pub const RHYTHM_STATE_VALUE_KIND: &str = "time/rhythm-state@1";
pub const PULSE_OBSERVE_KIND: &str = "time/pulse-observe";
pub const PHASE_SYNCHRONIZE_KIND: &str = "time/phase-synchronize";
pub const PULSE_OBSERVE_REVISION: &str = "conduit.std/time-pulse-observe@2";
pub const PHASE_SYNCHRONIZE_REVISION: &str = "conduit.std/time-phase-synchronize@1";
pub const PULSE_OBSERVATION_ENCODED_LEN: usize = 6;
pub const RHYTHM_STATE_ENCODED_LEN: usize = 14;
pub const MINIMUM_PERIOD_MS: u16 = 160;
pub const MAXIMUM_PERIOD_MS: u16 = 960;
pub const MAXIMUM_PHASE_ADJUSTMENT_MS: i16 = 64;
pub const MAXIMUM_PERIOD_ADJUSTMENT_MS: i16 = 16;
pub const SYNCHRONIZATION_WINDOW_MS: i16 = 320;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PulseObservation {
    pub sequence: u32,
    pub period_ms: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RhythmState {
    pub sequence: u32,
    pub next_pulse_at_ms: u32,
    pub period_ms: u16,
    pub expected_peer_sequence: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationOutcome {
    Adjusted { phase_ms: i16, period_ms: i16 },
    OutsideWindow,
    Stale,
    Missing,
    Pressure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RhythmError {
    WrongObservationLength(usize),
    WrongStateLength(usize),
    PeriodOutsideBounds(u16),
}

pub fn encode_pulse_observation(value: PulseObservation) -> [u8; PULSE_OBSERVATION_ENCODED_LEN] {
    let mut bytes = [0; PULSE_OBSERVATION_ENCODED_LEN];
    bytes[..4].copy_from_slice(&value.sequence.to_le_bytes());
    bytes[4..].copy_from_slice(&value.period_ms.to_le_bytes());
    bytes
}

pub fn decode_pulse_observation(bytes: &[u8]) -> Result<PulseObservation, RhythmError> {
    if bytes.len() != PULSE_OBSERVATION_ENCODED_LEN {
        return Err(RhythmError::WrongObservationLength(bytes.len()));
    }
    let value = PulseObservation {
        sequence: u32::from_le_bytes(bytes[..4].try_into().unwrap()),
        period_ms: u16::from_le_bytes(bytes[4..].try_into().unwrap()),
    };
    validate_period(value.period_ms)?;
    Ok(value)
}

pub fn encode_rhythm_state(value: RhythmState) -> [u8; RHYTHM_STATE_ENCODED_LEN] {
    let mut bytes = [0; RHYTHM_STATE_ENCODED_LEN];
    bytes[..4].copy_from_slice(&value.sequence.to_le_bytes());
    bytes[4..8].copy_from_slice(&value.next_pulse_at_ms.to_le_bytes());
    bytes[8..10].copy_from_slice(&value.period_ms.to_le_bytes());
    bytes[10..14].copy_from_slice(&value.expected_peer_sequence.to_le_bytes());
    bytes
}

pub fn decode_rhythm_state(bytes: &[u8]) -> Result<RhythmState, RhythmError> {
    if bytes.len() != RHYTHM_STATE_ENCODED_LEN {
        return Err(RhythmError::WrongStateLength(bytes.len()));
    }
    let value = RhythmState {
        sequence: u32::from_le_bytes(bytes[..4].try_into().unwrap()),
        next_pulse_at_ms: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        period_ms: u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
        expected_peer_sequence: u32::from_le_bytes(bytes[10..14].try_into().unwrap()),
    };
    validate_period(value.period_ms)?;
    Ok(value)
}

/// Applies one already-admitted peer observation. Arrival time is supplied by the
/// realization's exact monotonic clock; it is deliberately absent from authored meaning.
pub fn synchronize(
    state: &mut RhythmState,
    peer: PulseObservation,
    arrival_ms: u32,
) -> Result<SynchronizationOutcome, RhythmError> {
    validate_period(state.period_ms)?;
    validate_period(peer.period_ms)?;
    if peer.sequence != state.expected_peer_sequence {
        return Ok(SynchronizationOutcome::Stale);
    }
    state.expected_peer_sequence = state.expected_peer_sequence.wrapping_add(1);

    let phase_error = signed_distance(state.next_pulse_at_ms, arrival_ms);
    if phase_error.unsigned_abs() > SYNCHRONIZATION_WINDOW_MS as u32 {
        return Ok(SynchronizationOutcome::OutsideWindow);
    }
    let phase_adjustment = clamp_i32(
        rounded_half(phase_error),
        -i32::from(MAXIMUM_PHASE_ADJUSTMENT_MS),
        i32::from(MAXIMUM_PHASE_ADJUSTMENT_MS),
    ) as i16;
    let period_delta = i32::from(peer.period_ms) - i32::from(state.period_ms);
    let period_adjustment = clamp_i32(
        rounded_quarter(period_delta),
        -i32::from(MAXIMUM_PERIOD_ADJUSTMENT_MS),
        i32::from(MAXIMUM_PERIOD_ADJUSTMENT_MS),
    ) as i16;
    state.next_pulse_at_ms = state
        .next_pulse_at_ms
        .wrapping_add_signed(i32::from(phase_adjustment));
    state.period_ms = (i32::from(state.period_ms) + i32::from(period_adjustment))
        .clamp(i32::from(MINIMUM_PERIOD_MS), i32::from(MAXIMUM_PERIOD_MS))
        as u16;
    Ok(SynchronizationOutcome::Adjusted {
        phase_ms: phase_adjustment,
        period_ms: period_adjustment,
    })
}

pub fn missing_outcome() -> SynchronizationOutcome {
    SynchronizationOutcome::Missing
}

pub fn pressure_outcome() -> SynchronizationOutcome {
    SynchronizationOutcome::Pressure
}

fn validate_period(period_ms: u16) -> Result<(), RhythmError> {
    if (MINIMUM_PERIOD_MS..=MAXIMUM_PERIOD_MS).contains(&period_ms) {
        Ok(())
    } else {
        Err(RhythmError::PeriodOutsideBounds(period_ms))
    }
}

fn signed_distance(origin: u32, target: u32) -> i32 {
    target.wrapping_sub(origin) as i32
}

fn rounded_half(value: i32) -> i32 {
    if value >= 0 {
        (value + 1) / 2
    } else {
        (value - 1) / 2
    }
}

fn rounded_quarter(value: i32) -> i32 {
    if value >= 0 {
        (value + 2) / 4
    } else {
        (value - 2) / 4
    }
}

fn clamp_i32(value: i32, minimum: i32, maximum: i32) -> i32 {
    value.clamp(minimum, maximum)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(next: u32, period: u16, expected: u32) -> RhythmState {
        RhythmState {
            sequence: 7,
            next_pulse_at_ms: next,
            period_ms: period,
            expected_peer_sequence: expected,
        }
    }

    #[test]
    fn codecs_are_exact_and_reject_invalid_values() {
        let pulse = PulseObservation {
            sequence: 42,
            period_ms: 240,
        };
        assert_eq!(
            decode_pulse_observation(&encode_pulse_observation(pulse)),
            Ok(pulse)
        );
        let rhythm = state(1_024, 240, 3);
        assert_eq!(
            decode_rhythm_state(&encode_rhythm_state(rhythm)),
            Ok(rhythm)
        );
        assert_eq!(
            decode_pulse_observation(&[0; 5]),
            Err(RhythmError::WrongObservationLength(5))
        );
        assert_eq!(
            decode_pulse_observation(&encode_pulse_observation(PulseObservation {
                sequence: 0,
                period_ms: 1
            })),
            Err(RhythmError::PeriodOutsideBounds(1))
        );
    }

    #[test]
    fn asymmetric_adjustments_are_bounded_and_deterministic() {
        let mut late = state(1_000, 240, 4);
        assert_eq!(
            synchronize(
                &mut late,
                PulseObservation {
                    sequence: 4,
                    period_ms: 320
                },
                1_100
            ),
            Ok(SynchronizationOutcome::Adjusted {
                phase_ms: 50,
                period_ms: 16
            })
        );
        assert_eq!((late.next_pulse_at_ms, late.period_ms), (1_050, 256));
        let mut early = state(1_000, 240, 4);
        assert_eq!(
            synchronize(
                &mut early,
                PulseObservation {
                    sequence: 4,
                    period_ms: 160
                },
                900
            ),
            Ok(SynchronizationOutcome::Adjusted {
                phase_ms: -50,
                period_ms: -16
            })
        );
        assert_eq!((early.next_pulse_at_ms, early.period_ms), (950, 224));
    }

    #[test]
    fn stale_missing_pressure_and_outside_window_remain_distinct() {
        let mut rhythm = state(1_000, 240, 9);
        assert_eq!(
            synchronize(
                &mut rhythm,
                PulseObservation {
                    sequence: 8,
                    period_ms: 240
                },
                1_000
            ),
            Ok(SynchronizationOutcome::Stale)
        );
        assert_eq!(
            synchronize(
                &mut rhythm,
                PulseObservation {
                    sequence: 9,
                    period_ms: 240
                },
                1_400
            ),
            Ok(SynchronizationOutcome::OutsideWindow)
        );
        assert_eq!(missing_outcome(), SynchronizationOutcome::Missing);
        assert_eq!(pressure_outcome(), SynchronizationOutcome::Pressure);
    }
}
