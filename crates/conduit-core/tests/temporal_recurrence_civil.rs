use conduit_core::{
    CivilFoldPolicy, CivilGapPolicy, CivilOccurrenceResolution, CivilResolutionChoice,
    CivilResolutionPolicy, LocalDate, LocalDateTime, LocalTime, NamedTimeZone, OccurrenceInstant,
    RecurrenceDefinition, RecurrenceExpansion, RecurrenceRefusal, RecurrenceRule, RecurrenceUntil,
    RecurrenceWindow, TemporalInstant, TemporalScale, UtcOffsetSeconds, WeekdaySet,
    ZonedResolution, UNIX_UTC_CLOCK_BASIS,
};

fn zone(rule_set: &str) -> NamedTimeZone {
    NamedTimeZone::new("America/Los_Angeles".into(), rule_set.into()).unwrap()
}

fn local(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> LocalDateTime {
    LocalDateTime::new(
        LocalDate::new(year, month, day).unwrap(),
        LocalTime::new(hour, minute, 0, 0).unwrap(),
    )
}

fn instant(local: LocalDateTime, offset_seconds: i32) -> TemporalInstant {
    local
        .to_offset_instant(
            UtcOffsetSeconds::new(offset_seconds).unwrap(),
            TemporalScale::Seconds,
        )
        .unwrap()
}

fn window(start: u64, end: u64, maximum_results: u32) -> RecurrenceExpansion {
    let at = |ticks| TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: UNIX_UTC_CLOCK_BASIS.into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    };
    RecurrenceExpansion {
        maximum_results,
        window: RecurrenceWindow::Wall {
            start: at(start),
            end: at(end),
        },
    }
}

fn policy(gap: CivilGapPolicy, fold: CivilFoldPolicy) -> CivilResolutionPolicy {
    CivilResolutionPolicy { gap, fold }
}

#[test]
fn weekday_meeting_preserves_nine_am_across_spring_and_fall_dst() {
    let pacific = zone("tzdb/2026a");
    let march = local(2026, 3, 2, 9, 0);
    let july = local(2026, 7, 6, 9, 0);
    let november = local(2026, 11, 2, 9, 0);
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/weekly-human-meeting".into(),
        rule: RecurrenceRule::CivilWeekdays {
            first_date: march.date,
            local_time: march.time,
            zone: pacific.clone(),
            weekdays: WeekdaySet::MONDAY,
            excluded_dates: vec![],
        },
        maximum_occurrences: 36,
        until: None,
        excluded_ordinals: (1..18).chain(19..35).collect(),
    };
    let march_instant = instant(march, -8 * 3_600);
    let july_instant = instant(july, -7 * 3_600);
    let november_instant = instant(november, -8 * 3_600);
    let resolutions = vec![
        CivilOccurrenceResolution {
            ordinal: 0,
            resolution: ZonedResolution::Unique {
                local: march,
                zone: pacific.clone(),
                instant: march_instant.clone(),
            },
        },
        CivilOccurrenceResolution {
            ordinal: 18,
            resolution: ZonedResolution::Unique {
                local: july,
                zone: pacific.clone(),
                instant: july_instant.clone(),
            },
        },
        CivilOccurrenceResolution {
            ordinal: 35,
            resolution: ZonedResolution::Unique {
                local: november,
                zone: pacific.clone(),
                instant: november_instant.clone(),
            },
        },
    ];
    let occurrences = recurrence
        .expand_civil(
            &window(march_instant.ticks, november_instant.ticks, 3),
            &resolutions,
            policy(CivilGapPolicy::Refuse, CivilFoldPolicy::Refuse),
        )
        .unwrap();
    assert_eq!(occurrences.len(), 3);
    for (occurrence, expected_local) in occurrences.iter().zip([march, july, november]) {
        let OccurrenceInstant::Civil {
            local,
            zone,
            resolution,
            ..
        } = &occurrence.at
        else {
            panic!("civil recurrence lost its basis")
        };
        assert_eq!(*local, expected_local);
        assert_eq!(local.time.hour(), 9);
        assert_eq!(zone.rule_set(), "tzdb/2026a");
        assert_eq!(*resolution, CivilResolutionChoice::Unique);
    }
    assert_eq!(
        july_instant.ticks - march_instant.ticks,
        18 * 7 * 24 * 60 * 60 - 3_600
    );
    assert_eq!(
        november_instant.ticks - july_instant.ticks,
        17 * 7 * 24 * 60 * 60 + 3_600
    );
}

#[test]
fn gap_and_fold_policy_are_explicit_and_fold_both_has_unique_identities() {
    let pacific = zone("tzdb/2026a");
    let gap_local = local(2026, 3, 8, 2, 30);
    let gap_before = instant(local(2026, 3, 8, 1, 59), -8 * 3_600);
    let gap_after = instant(local(2026, 3, 8, 3, 0), -7 * 3_600);
    let gap = RecurrenceDefinition {
        identity: "recurrence/dst-gap".into(),
        rule: RecurrenceRule::CivilWeekdays {
            first_date: gap_local.date,
            local_time: gap_local.time,
            zone: pacific.clone(),
            weekdays: WeekdaySet::SUNDAY,
            excluded_dates: vec![],
        },
        maximum_occurrences: 1,
        until: None,
        excluded_ordinals: vec![],
    };
    let gap_truth = [CivilOccurrenceResolution {
        ordinal: 0,
        resolution: ZonedResolution::Nonexistent {
            local: gap_local,
            zone: pacific.clone(),
            gap_before: gap_before.clone(),
            gap_after: gap_after.clone(),
        },
    }];
    let request = window(gap_before.ticks, gap_after.ticks, 1);
    assert_eq!(
        gap.expand_civil(
            &request,
            &gap_truth,
            policy(CivilGapPolicy::Refuse, CivilFoldPolicy::Earlier)
        ),
        Err(RecurrenceRefusal::CivilResolutionRequired)
    );
    let shifted = gap
        .expand_civil(
            &request,
            &gap_truth,
            policy(CivilGapPolicy::UseAfter, CivilFoldPolicy::Earlier),
        )
        .unwrap();
    assert!(shifted[0].identity.ends_with("/gap/after"));

    let fold_local = local(2026, 11, 1, 1, 30);
    let earlier = instant(fold_local, -7 * 3_600);
    let later = instant(fold_local, -8 * 3_600);
    let fold = RecurrenceDefinition {
        identity: "recurrence/dst-fold".into(),
        rule: RecurrenceRule::CivilWeekdays {
            first_date: fold_local.date,
            local_time: fold_local.time,
            zone: pacific.clone(),
            weekdays: WeekdaySet::SUNDAY,
            excluded_dates: vec![],
        },
        maximum_occurrences: 1,
        until: None,
        excluded_ordinals: vec![],
    };
    let both = fold
        .expand_civil(
            &window(earlier.ticks, later.ticks, 2),
            &[CivilOccurrenceResolution {
                ordinal: 0,
                resolution: ZonedResolution::Ambiguous {
                    local: fold_local,
                    zone: pacific,
                    earlier,
                    later,
                },
            }],
            policy(CivilGapPolicy::Skip, CivilFoldPolicy::Both),
        )
        .unwrap();
    assert_eq!(both.len(), 2);
    assert_ne!(both[0].identity, both[1].identity);
    assert!(both[0].identity.ends_with("/fold/earlier"));
    assert!(both[1].identity.ends_with("/fold/later"));
}

#[test]
fn exceptions_need_no_resolution_and_ruleset_mutation_refuses() {
    let original_zone = zone("tzdb/2026a");
    let first = local(2026, 3, 2, 9, 0);
    let second = local(2026, 3, 9, 9, 0);
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/cancelled-meeting".into(),
        rule: RecurrenceRule::CivilWeekdays {
            first_date: first.date,
            local_time: first.time,
            zone: original_zone,
            weekdays: WeekdaySet::MONDAY,
            excluded_dates: vec![first.date],
        },
        maximum_occurrences: 3,
        until: Some(RecurrenceUntil::CivilDate(second.date)),
        excluded_ordinals: vec![],
    };
    let resolved = instant(second, -7 * 3_600);
    let request = window(resolved.ticks, resolved.ticks, 1);
    let wrong_rules = [CivilOccurrenceResolution {
        ordinal: 1,
        resolution: ZonedResolution::Unique {
            local: second,
            zone: zone("tzdb/2026b"),
            instant: resolved.clone(),
        },
    }];
    assert_eq!(
        recurrence.expand_civil(
            &request,
            &wrong_rules,
            policy(CivilGapPolicy::Refuse, CivilFoldPolicy::Refuse)
        ),
        Err(RecurrenceRefusal::CivilResolutionMismatch)
    );
    let exact_rules = [CivilOccurrenceResolution {
        ordinal: 1,
        resolution: ZonedResolution::Unique {
            local: second,
            zone: zone("tzdb/2026a"),
            instant: resolved,
        },
    }];
    let occurrences = recurrence
        .expand_civil(
            &request,
            &exact_rules,
            policy(CivilGapPolicy::Refuse, CivilFoldPolicy::Refuse),
        )
        .unwrap();
    assert_eq!(occurrences.len(), 1);
    assert_eq!(occurrences[0].ordinal, 1);
}

#[test]
fn missing_duplicate_and_result_overflow_never_return_partial_state() {
    let pacific = zone("tzdb/2026a");
    let first = local(2026, 11, 1, 1, 30);
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/fold-bound".into(),
        rule: RecurrenceRule::CivilWeekdays {
            first_date: first.date,
            local_time: first.time,
            zone: pacific.clone(),
            weekdays: WeekdaySet::SUNDAY,
            excluded_dates: vec![],
        },
        maximum_occurrences: 1,
        until: None,
        excluded_ordinals: vec![],
    };
    assert_eq!(
        recurrence.expand_civil(
            &window(0, u64::MAX, 1),
            &[],
            policy(CivilGapPolicy::Skip, CivilFoldPolicy::Earlier)
        ),
        Err(RecurrenceRefusal::CivilResolutionRequired)
    );
    let truth = CivilOccurrenceResolution {
        ordinal: 0,
        resolution: ZonedResolution::Ambiguous {
            local: first,
            zone: pacific,
            earlier: instant(first, -7 * 3_600),
            later: instant(first, -8 * 3_600),
        },
    };
    assert_eq!(
        recurrence.expand_civil(
            &window(0, u64::MAX, 2),
            &[truth.clone(), truth.clone()],
            policy(CivilGapPolicy::Skip, CivilFoldPolicy::Both)
        ),
        Err(RecurrenceRefusal::InvalidCivilResolution)
    );
    assert_eq!(
        recurrence.expand_civil(
            &window(0, u64::MAX, 1),
            &[truth],
            policy(CivilGapPolicy::Skip, CivilFoldPolicy::Both)
        ),
        Err(RecurrenceRefusal::WorkLimitExceeded)
    );
}

#[test]
fn exception_date_must_name_an_actual_selected_civil_occurrence() {
    let first = local(2026, 3, 2, 9, 0);
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/invalid-exception".into(),
        rule: RecurrenceRule::CivilWeekdays {
            first_date: first.date,
            local_time: first.time,
            zone: zone("tzdb/2026a"),
            weekdays: WeekdaySet::MONDAY,
            excluded_dates: vec![LocalDate::new(2026, 3, 3).unwrap()],
        },
        maximum_occurrences: 2,
        until: None,
        excluded_ordinals: vec![],
    };
    assert_eq!(
        recurrence.validate(),
        Err(RecurrenceRefusal::InvalidExceptions)
    );
}
