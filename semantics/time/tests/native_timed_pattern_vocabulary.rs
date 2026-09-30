use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::TimedPatternRefusal;

#[test]
fn timed_pattern_refusals_round_trip_through_their_exact_native_type() {
    for refusal in [
        TimedPatternRefusal::Malformed,
        TimedPatternRefusal::TooFewEvents,
        TimedPatternRefusal::TooManyEvents,
        TimedPatternRefusal::ReorderedOrDuplicateEvent,
        TimedPatternRefusal::IntervalOverflow,
    ] {
        let structured = refusal.into_structured().unwrap();
        assert_eq!(
            TimedPatternRefusal::from_structured(structured).unwrap(),
            refusal
        );
    }
}
