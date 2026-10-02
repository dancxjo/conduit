use conduit_plot::rust_binding::NativeRustBinding;
use conduit_time::{RhythmState, SynchronizationOutcome};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn rhythm_state_round_trips_its_exact_full_scalar_domain() {
    for state in [
        RhythmState {
            sequence: 0,
            next_pulse_at_ms: 0,
            period_ms: 0,
            expected_peer_sequence: 0,
        },
        RhythmState {
            sequence: u32::MAX,
            next_pulse_at_ms: u32::MAX,
            period_ms: u16::MAX,
            expected_peer_sequence: u32::MAX,
        },
    ] {
        assert_round_trip(state);
    }
}

#[test]
fn synchronization_outcomes_round_trip_exact_adjustment_boundaries() {
    for outcome in [
        SynchronizationOutcome::adjusted(i16::MIN, i16::MAX).unwrap(),
        SynchronizationOutcome::OutsideWindow,
        SynchronizationOutcome::Stale,
        SynchronizationOutcome::Missing,
        SynchronizationOutcome::Pressure,
    ] {
        assert_round_trip(outcome);
    }
}
