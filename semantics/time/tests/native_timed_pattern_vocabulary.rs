use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::{
    ScheduleRefusal, TemporalWindowPosition, TimedPatternRefusal, WorkflowLifecycle,
};

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

#[test]
fn schedule_vocabulary_uses_exact_native_types() {
    for lifecycle in [
        WorkflowLifecycle::Pending,
        WorkflowLifecycle::Running,
        WorkflowLifecycle::Completed,
        WorkflowLifecycle::Failed,
        WorkflowLifecycle::Cancelled,
        WorkflowLifecycle::Expired,
    ] {
        let structured = lifecycle.into_structured().unwrap();
        assert_eq!(
            WorkflowLifecycle::from_structured(structured).unwrap(),
            lifecycle
        );
    }

    for refusal in [
        ScheduleRefusal::NonTemporalQuantity,
        ScheduleRefusal::NegativeQuantity,
        ScheduleRefusal::InconsistentLifecycle,
    ] {
        let structured = refusal.into_structured().unwrap();
        assert_eq!(
            ScheduleRefusal::from_structured(structured).unwrap(),
            refusal
        );
    }

    for position in [
        TemporalWindowPosition::Before,
        TemporalWindowPosition::Within,
        TemporalWindowPosition::After,
        TemporalWindowPosition::Indeterminate,
    ] {
        let structured = position.into_structured().unwrap();
        assert_eq!(
            TemporalWindowPosition::from_structured(structured).unwrap(),
            position
        );
    }
}
