use conduit_core::{
    elapsed_trigger_window, BootId, CivilTrigger, ClockChangeBehavior, HostId,
    MissedOccurrencePolicy, MonotonicClockIdentity, MonotonicDuration, MonotonicInstant,
    NamedTimeZone, OccurrenceInstant, RecurrenceOccurrence, ScheduledIntent,
    ScheduledIntentRefusal, ScheduledOccurrenceDecision, SuspendBehavior, TemporalBoundary,
    TemporalInstant, TemporalScale, TemporalWindow, TriggerObservation, TriggerProfile,
};

#[test]
fn elapsed_job_is_ready_without_becoming_effect_authority() {
    let clock = clock("boot/one");
    let opens = MonotonicInstant::new(100, clock.clone()).unwrap();
    let occurrence = recurrence(OccurrenceInstant::Monotonic(opens.clone()));
    let intent = ScheduledIntent {
        identity: "scheduled/job/report#0".into(),
        occurrence,
        trigger: TriggerProfile::Elapsed(
            elapsed_trigger_window(
                opens,
                MonotonicDuration::new(10, TemporalScale::Milliseconds),
                SuspendBehavior::ClockIncludesSuspend,
            )
            .unwrap(),
        ),
        missed: MissedOccurrencePolicy::Expire,
        payload: "process/job-request@1",
    };
    assert_eq!(
        intent
            .decide(
                &TriggerObservation::Elapsed {
                    now: MonotonicInstant::new(104, clock).unwrap(),
                    suspend_observed: false,
                },
                false,
            )
            .unwrap(),
        ScheduledOccurrenceDecision::Ready { lateness_ticks: 4 }
    );
    assert_eq!(intent.payload, "process/job-request@1");
}

#[test]
fn elapsed_profile_makes_reboot_suspend_and_missed_policy_explicit() {
    let first_clock = clock("boot/one");
    let opens = MonotonicInstant::new(100, first_clock.clone()).unwrap();
    let mut intent = ScheduledIntent {
        identity: "scheduled/job/report#0".into(),
        occurrence: recurrence(OccurrenceInstant::Monotonic(opens.clone())),
        trigger: TriggerProfile::Elapsed(
            elapsed_trigger_window(
                opens,
                MonotonicDuration::new(10, TemporalScale::Milliseconds),
                SuspendBehavior::RefuseAfterSuspend,
            )
            .unwrap(),
        ),
        missed: MissedOccurrencePolicy::FireLate {
            maximum_lateness_ticks: 5,
        },
        payload: (),
    };
    assert_eq!(
        decide_elapsed(&intent, 105, first_clock.clone(), true),
        ScheduledOccurrenceDecision::Suspended
    );
    assert_eq!(
        decide_elapsed(&intent, 112, first_clock.clone(), false),
        ScheduledOccurrenceDecision::Ready { lateness_ticks: 2 }
    );
    assert_eq!(
        decide_elapsed(&intent, 116, first_clock, false),
        ScheduledOccurrenceDecision::Missed
    );
    assert_eq!(
        decide_elapsed(&intent, 105, clock("boot/two"), false),
        ScheduledOccurrenceDecision::Rebooted
    );
    intent.missed = MissedOccurrencePolicy::Expire;
    assert_eq!(
        decide_elapsed(&intent, 116, clock("boot/one"), false),
        ScheduledOccurrenceDecision::Expired
    );
}

#[test]
fn civil_reminder_preserves_zone_and_clock_change_policy() {
    let start = wall(1_000, 0);
    let end = wall(1_100, 0);
    let zone = NamedTimeZone::new("America/Los_Angeles".into(), "tzdb/2026b".into()).unwrap();
    let intent = ScheduledIntent {
        identity: "scheduled/reminder/meeting#0".into(),
        occurrence: recurrence(OccurrenceInstant::Wall(start.clone())),
        trigger: TriggerProfile::Civil(CivilTrigger {
            window: TemporalWindow::new(
                start,
                TemporalBoundary::Inclusive,
                end,
                TemporalBoundary::Inclusive,
            )
            .unwrap(),
            zone: zone.clone(),
            clock_change: ClockChangeBehavior::RefuseAfterChange,
        }),
        missed: MissedOccurrencePolicy::Skip,
        payload: "notification/reminder@1",
    };
    assert_eq!(
        intent
            .decide(
                &TriggerObservation::Civil {
                    now: wall(1_020, 0),
                    clock_change_observed: false,
                },
                false,
            )
            .unwrap(),
        ScheduledOccurrenceDecision::Ready { lateness_ticks: 20 }
    );
    assert_eq!(
        intent
            .decide(
                &TriggerObservation::Civil {
                    now: wall(1_020, 0),
                    clock_change_observed: true,
                },
                false,
            )
            .unwrap(),
        ScheduledOccurrenceDecision::ClockChanged
    );
    assert_eq!(
        intent
            .decide(
                &TriggerObservation::Civil {
                    now: wall(1_020, 1),
                    clock_change_observed: false,
                },
                false,
            )
            .unwrap(),
        ScheduledOccurrenceDecision::ClockUncertain
    );
    let TriggerProfile::Civil(trigger) = &intent.trigger else {
        panic!("civil trigger")
    };
    assert_eq!(trigger.zone, zone);
}

#[test]
fn observation_profiles_cannot_be_substituted() {
    let initial_clock = clock("boot/one");
    let opens = MonotonicInstant::new(100, initial_clock).unwrap();
    let intent = ScheduledIntent {
        identity: "scheduled/job/report#0".into(),
        occurrence: recurrence(OccurrenceInstant::Monotonic(opens.clone())),
        trigger: TriggerProfile::Elapsed(
            elapsed_trigger_window(
                opens,
                MonotonicDuration::new(10, TemporalScale::Milliseconds),
                SuspendBehavior::ClockExcludesSuspend,
            )
            .unwrap(),
        ),
        missed: MissedOccurrencePolicy::Skip,
        payload: (),
    };
    assert_eq!(
        intent.decide(
            &TriggerObservation::Civil {
                now: wall(100, 0),
                clock_change_observed: false,
            },
            false,
        ),
        Err(ScheduledIntentRefusal::WrongObservationProfile)
    );
    assert_eq!(
        intent
            .decide(
                &TriggerObservation::Elapsed {
                    now: MonotonicInstant::new(100, clock("boot/one")).unwrap(),
                    suspend_observed: false,
                },
                true,
            )
            .unwrap(),
        ScheduledOccurrenceDecision::Cancelled
    );
}

fn decide_elapsed(
    intent: &ScheduledIntent<()>,
    ticks: u64,
    clock: MonotonicClockIdentity,
    suspend_observed: bool,
) -> ScheduledOccurrenceDecision {
    intent
        .decide(
            &TriggerObservation::Elapsed {
                now: MonotonicInstant::new(ticks, clock).unwrap(),
                suspend_observed,
            },
            false,
        )
        .unwrap()
}

fn clock(boot: &str) -> MonotonicClockIdentity {
    MonotonicClockIdentity::new(
        HostId::from("host/schedule"),
        BootId::from(boot),
        "std/monotonic@1".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap()
}

fn wall(ticks: u64, uncertainty_ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: "unix/utc@1".into(),
        resolution_ticks: 1,
        uncertainty_ticks,
    }
}

fn recurrence(at: OccurrenceInstant) -> RecurrenceOccurrence {
    RecurrenceOccurrence {
        identity: "recurrence/shared/occurrence/0".into(),
        recurrence_identity: "recurrence/shared".into(),
        ordinal: 0,
        at,
    }
}
