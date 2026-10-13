use conduit_core::{
    BootId, HostId, MonotonicClockIdentity, MonotonicInstant, MonotonicTimeRefusal,
    TemporalInstant as CoreInstant, TemporalScale as CoreScale,
};
use conduit_time::{
    CivilDeadlineAdmission, CivilDeadlineRefusal, CivilResolutionChoice, ClockCorrelation,
    LocalDate, LocalDateTime, LocalTime, NamedTimeZone, OccurrenceInstant, TemporalInstant,
    TemporalScale, ZonedResolution,
};

fn wall(ticks: u64) -> CoreInstant {
    CoreInstant {
        ticks,
        scale: CoreScale::Milliseconds,
        clock_basis: "time/unix-utc@1".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn clock(boot: &str, basis: &str, ticks: u64) -> MonotonicInstant {
    MonotonicInstant::new(
        ticks,
        MonotonicClockIdentity::new(
            HostId::from("host/a"),
            BootId::from(boot),
            basis.into(),
            CoreScale::Milliseconds,
            1,
            0,
        )
        .unwrap(),
    )
    .unwrap()
}

fn civil(choice: CivilResolutionChoice, ticks: u64) -> OccurrenceInstant {
    OccurrenceInstant::civil(
        TemporalInstant::new(
            "time/unix-utc@1".into(),
            1,
            TemporalScale::Milliseconds,
            ticks,
            0,
        )
        .unwrap(),
        LocalDateTime::new(
            LocalDate::new(2026, 11, 1).unwrap(),
            LocalTime::new(1, 30, 0, 0).unwrap(),
        )
        .unwrap(),
        choice,
        NamedTimeZone::new("America/Los_Angeles".into(), "tzdb/2026b".into()).unwrap(),
    )
    .unwrap()
}

fn resolution() -> ZonedResolution {
    ZonedResolution::Ambiguous {
        local: conduit_core::LocalDateTime::new(
            conduit_core::LocalDate::new(2026, 11, 1).unwrap(),
            conduit_core::LocalTime::new(1, 30, 0, 0).unwrap(),
        ),
        zone: conduit_core::NamedTimeZone::new("America/Los_Angeles".into(), "tzdb/2026b".into())
            .unwrap(),
        earlier: wall(2_000),
        later: wall(3_000),
    }
}

fn correlation() -> ClockCorrelation {
    ClockCorrelation::new(
        "sample/1".into(),
        clock("boot/1", "steady/1", 100),
        wall(1_000),
        2,
    )
    .unwrap()
}

#[test]
fn fold_choice_and_provenance_survive_monotonic_admission() {
    let admitted = CivilDeadlineAdmission::admit(
        civil(CivilResolutionChoice::FoldLater, 3_000),
        resolution(),
        correlation(),
        0,
        2,
        10_000,
    )
    .unwrap();
    assert_eq!(admitted.deadline.instant().ticks(), 2_102);
    assert_eq!(admitted.uncertainty_ticks, 2);
    assert_eq!(admitted.correlation.identity(), "sample/1");
    assert_eq!(
        admitted
            .remaining_at(&clock("boot/1", "steady/1", 2_101))
            .unwrap()
            .unwrap()
            .ticks(),
        1
    );
    assert_eq!(
        admitted.remaining_at(&clock("boot/1", "steady/1", 2_102)),
        Ok(None)
    );
    assert_eq!(
        admitted.remaining_at(&clock("boot/2", "steady/1", 2_102)),
        Err(MonotonicTimeRefusal::DifferentClock)
    );
    assert_eq!(
        admitted.remaining_at(&clock("boot/1", "steady/2", 2_102)),
        Err(MonotonicTimeRefusal::DifferentClock)
    );
}

#[test]
fn resolution_and_quality_mismatches_refuse() {
    assert_eq!(
        CivilDeadlineAdmission::admit(
            civil(CivilResolutionChoice::FoldEarlier, 3_000),
            resolution(),
            correlation(),
            0,
            2,
            10_000
        ),
        Err(CivilDeadlineRefusal::ResolutionMismatch)
    );
    assert_eq!(
        CivilDeadlineAdmission::admit(
            civil(CivilResolutionChoice::FoldLater, 3_000),
            resolution(),
            correlation(),
            0,
            1,
            10_000
        ),
        Err(CivilDeadlineRefusal::UncertaintyExceeded)
    );
}
