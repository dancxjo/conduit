use conduit_core::{Quantity, QuantityUnit};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_time::{
    ScheduleRefusal, TemporalWindowPosition, TimedPatternRefusal, WorkflowLifecycle,
    WorkflowTimingOutcome,
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

#[test]
fn workflow_timing_outcomes_keep_duration_payloads_in_the_native_type() {
    let duration = Quantity::new(2, QuantityUnit::Second);
    for outcome in [
        WorkflowTimingOutcome::Awaiting,
        WorkflowTimingOutcome::OnTime,
        WorkflowTimingOutcome::late(duration).unwrap(),
        WorkflowTimingOutcome::MissedWindow,
        WorkflowTimingOutcome::clock_uncertain(duration).unwrap(),
        WorkflowTimingOutcome::Failed,
        WorkflowTimingOutcome::Cancelled,
        WorkflowTimingOutcome::Expired,
    ] {
        let structured = outcome.clone().into_structured().unwrap();
        assert_eq!(
            WorkflowTimingOutcome::from_structured(structured).unwrap(),
            outcome
        );
    }
}
