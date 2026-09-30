use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::ReplayPolicy;

fn assert_round_trip(value: ReplayPolicy) {
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(ReplayPolicy::from_structured(structured).unwrap(), value);
}

#[test]
fn replay_policy_round_trips_through_its_exact_native_type() {
    assert_round_trip(ReplayPolicy::step());
    assert_round_trip(ReplayPolicy::original_timing());
    assert_round_trip(ReplayPolicy::rate(3, 2).unwrap());
}

#[test]
fn replay_rate_bounds_belong_to_the_language_type() {
    assert!(ReplayPolicy::rate(1, 0).is_err());
    assert!(ReplayPolicy::rate(0, 1).is_err());
    assert!(ReplayPolicy::rate(1, 1_001).is_err());
    assert!(ReplayPolicy::rate(1_001, 1).is_err());
}
