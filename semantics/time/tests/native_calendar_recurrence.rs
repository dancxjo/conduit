use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_time::{
    CalendarEventTime, LocalDate, LocalDateTime, LocalTime, NamedTimeZone, RecurrenceDefinition,
    RecurrenceRule, TemporalBoundary, TemporalInstant, TemporalScale, TemporalWindow,
    TimedCalendarSpan,
};

fn instant(ticks: u64) -> TemporalInstant {
    TemporalInstant::new("utc".into(), 1, TemporalScale::Seconds, ticks, 0).unwrap()
}

#[test]
fn calendar_and_recurrence_are_native_bounded_values() {
    assert!(LocalDate::new(2026, 13, 1).is_err());

    let start = instant(100);
    let end = instant(200);
    let window = TemporalWindow::new(
        start.clone(),
        TemporalBoundary::Inclusive,
        end,
        TemporalBoundary::Exclusive,
    )
    .unwrap();
    window.validate().unwrap();

    let local_start = LocalDateTime::new(
        LocalDate::new(2026, 10, 1).unwrap(),
        LocalTime::new(9, 0, 0, 0).unwrap(),
    )
    .unwrap();
    let local_end = LocalDateTime::new(
        LocalDate::new(2026, 10, 1).unwrap(),
        LocalTime::new(10, 0, 0, 0).unwrap(),
    )
    .unwrap();
    let span = TimedCalendarSpan {
        local_start,
        local_end,
        zone: NamedTimeZone::new("America/Los_Angeles".into(), "tzdb/2026a".into()).unwrap(),
        instant: window,
    };
    let event_time = CalendarEventTime::timed(span).unwrap();
    let structured = event_time.clone().into_structured().unwrap();
    assert_eq!(
        CalendarEventTime::from_structured(structured).unwrap(),
        event_time
    );

    let rule = RecurrenceRule::one_shot(start).unwrap();
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/one".into(),
        rule,
        maximum_occurrences: 1,
        until: None,
        excluded_ordinals: BoundedSequence::new(),
    };
    recurrence.validate().unwrap();
    let structured = recurrence.clone().into_structured().unwrap();
    assert_eq!(
        RecurrenceDefinition::from_structured(structured).unwrap(),
        recurrence
    );
}
