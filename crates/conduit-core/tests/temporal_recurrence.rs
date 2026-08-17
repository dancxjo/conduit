use conduit_core::{
    BootId, HostId, LocalDate, LocalTime, MonotonicClockIdentity, MonotonicDuration,
    MonotonicInstant, NamedTimeZone, OccurrenceInstant, RecurrenceDefinition, RecurrenceExpansion,
    RecurrenceRefusal, RecurrenceRule, RecurrenceUntil, RecurrenceWindow, TemporalInstant,
    TemporalScale, WeekdaySet,
};

fn clock(boot: &str) -> MonotonicClockIdentity {
    MonotonicClockIdentity::new(
        HostId::from("host/scheduler"),
        BootId::from(boot),
        "clock/monotonic".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap()
}

fn monotonic(ticks: u64) -> MonotonicInstant {
    MonotonicInstant::new(ticks, clock("boot/a")).unwrap()
}

fn wall(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/unix-epoch".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

#[test]
fn fixed_elapsed_expansion_is_bounded_deterministic_and_exception_aware() {
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/heartbeat".into(),
        rule: RecurrenceRule::FixedElapsed {
            first: monotonic(100),
            every: MonotonicDuration::new(10, TemporalScale::Milliseconds),
        },
        maximum_occurrences: 6,
        until: None,
        excluded_ordinals: vec![2, 4],
    };
    let occurrences = recurrence
        .expand(&RecurrenceExpansion {
            maximum_results: 4,
            window: RecurrenceWindow::Monotonic {
                start: monotonic(105),
                end: monotonic(150),
            },
        })
        .unwrap();
    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| occurrence.ordinal)
            .collect::<Vec<_>>(),
        vec![1, 3, 5]
    );
    assert_eq!(occurrences[0].identity, "recurrence/heartbeat/occurrence/1");
    assert!(matches!(occurrences[0].at, OccurrenceInstant::Monotonic(_)));
}

#[test]
fn one_shot_is_distinct_from_elapsed_repetition() {
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/deadline".into(),
        rule: RecurrenceRule::OneShot { at: wall(200) },
        maximum_occurrences: 1,
        until: None,
        excluded_ordinals: vec![],
    };
    let occurrences = recurrence
        .expand(&RecurrenceExpansion {
            maximum_results: 1,
            window: RecurrenceWindow::Wall {
                start: wall(100),
                end: wall(300),
            },
        })
        .unwrap();
    assert_eq!(occurrences.len(), 1);
    assert!(matches!(occurrences[0].at, OccurrenceInstant::Wall(_)));
}

#[test]
fn civil_weekday_rule_retains_zone_and_requires_explicit_resolution() {
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/weekday-meeting".into(),
        rule: RecurrenceRule::CivilWeekdays {
            first_date: LocalDate::new(2026, 3, 2).unwrap(),
            local_time: LocalTime::new(9, 0, 0, 0).unwrap(),
            zone: NamedTimeZone::new("America/Los_Angeles".into(), "tzdb/2026a".into()).unwrap(),
            weekdays: WeekdaySet::WEEKDAYS,
            excluded_dates: vec![],
        },
        maximum_occurrences: 10,
        until: None,
        excluded_ordinals: vec![3],
    };
    assert_eq!(
        recurrence.expand(&RecurrenceExpansion {
            maximum_results: 5,
            window: RecurrenceWindow::Wall {
                start: wall(0),
                end: wall(1_000),
            },
        }),
        Err(RecurrenceRefusal::CivilResolutionRequired)
    );
    let RecurrenceRule::CivilWeekdays { zone, weekdays, .. } = recurrence.rule else {
        panic!("civil rule changed kind")
    };
    assert_eq!(zone.rule_set(), "tzdb/2026a");
    assert!(weekdays.contains(WeekdaySet::MONDAY));
    assert!(!weekdays.contains(WeekdaySet::SUNDAY));
}

#[test]
fn work_window_clock_and_exception_bounds_fail_closed() {
    let mut recurrence = RecurrenceDefinition {
        identity: "recurrence/metronome".into(),
        rule: RecurrenceRule::FixedElapsed {
            first: monotonic(0),
            every: MonotonicDuration::new(1, TemporalScale::Milliseconds),
        },
        maximum_occurrences: 4,
        until: None,
        excluded_ordinals: vec![],
    };
    assert_eq!(
        recurrence.expand(&RecurrenceExpansion {
            maximum_results: 2,
            window: RecurrenceWindow::Monotonic {
                start: monotonic(0),
                end: monotonic(3),
            },
        }),
        Err(RecurrenceRefusal::WorkLimitExceeded)
    );
    assert_eq!(
        recurrence.expand(&RecurrenceExpansion {
            maximum_results: 4,
            window: RecurrenceWindow::Monotonic {
                start: monotonic(0),
                end: MonotonicInstant::new(3, clock("boot/b")).unwrap(),
            },
        }),
        Err(RecurrenceRefusal::IncomparableWindow)
    );
    recurrence.excluded_ordinals = vec![2, 2];
    assert_eq!(
        recurrence.validate(),
        Err(RecurrenceRefusal::InvalidExceptions)
    );
}

#[test]
fn arithmetic_overflow_refuses_without_partial_occurrences() {
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/overflow".into(),
        rule: RecurrenceRule::FixedElapsed {
            first: monotonic(u64::MAX - 1),
            every: MonotonicDuration::new(2, TemporalScale::Milliseconds),
        },
        maximum_occurrences: 2,
        until: None,
        excluded_ordinals: vec![],
    };
    assert_eq!(
        recurrence.expand(&RecurrenceExpansion {
            maximum_results: 2,
            window: RecurrenceWindow::Monotonic {
                start: monotonic(u64::MAX - 1),
                end: monotonic(u64::MAX),
            },
        }),
        Err(RecurrenceRefusal::ArithmeticOverflow)
    );
}

#[test]
fn elapsed_until_is_distinct_from_count_and_request_window() {
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/bounded-heartbeat".into(),
        rule: RecurrenceRule::FixedElapsed {
            first: monotonic(10),
            every: MonotonicDuration::new(10, TemporalScale::Milliseconds),
        },
        maximum_occurrences: 10,
        until: Some(RecurrenceUntil::Monotonic(monotonic(35))),
        excluded_ordinals: vec![],
    };
    let occurrences = recurrence
        .expand(&RecurrenceExpansion {
            maximum_results: 10,
            window: RecurrenceWindow::Monotonic {
                start: monotonic(0),
                end: monotonic(100),
            },
        })
        .unwrap();
    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| occurrence.ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    let mut wrong_basis = recurrence;
    wrong_basis.until = Some(RecurrenceUntil::Wall(wall(35)));
    assert_eq!(wrong_basis.validate(), Err(RecurrenceRefusal::InvalidRule));
}
