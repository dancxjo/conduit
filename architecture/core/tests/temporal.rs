use conduit_core::{
    CivilTimeBasis, CivilTimeRefusal, LocalDate, LocalDateTime, LocalTime, NamedTimeZone,
    TemporalInstant, TemporalRelation, TemporalRelationError, TemporalScale, UtcOffsetSeconds,
    ZonedResolution,
};

fn instant(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "unix-epoch".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

#[test]
fn shared_instant_relation_preserves_exact_basis_scale_and_uncertainty() {
    assert_eq!(
        instant(1_000).relation_to(&instant(1_025)),
        Ok(TemporalRelation::Past {
            minimum_ticks: 25,
            maximum_ticks: 25,
        })
    );
    let mut uncertain = instant(1_000);
    uncertain.uncertainty_ticks = 10;
    let mut reference = instant(1_005);
    reference.uncertainty_ticks = 10;
    assert_eq!(
        uncertain.relation_to(&reference),
        Ok(TemporalRelation::Indeterminate)
    );
    reference.clock_basis = "boot/other".into();
    assert_eq!(
        uncertain.relation_to(&reference),
        Err(TemporalRelationError::Incomparable)
    );
}

#[test]
fn gregorian_dates_and_local_times_refuse_invalid_boundaries() {
    assert!(LocalDate::new(2024, 2, 29).is_ok());
    assert_eq!(
        LocalDate::new(2025, 2, 29),
        Err(CivilTimeRefusal::InvalidDate)
    );
    assert_eq!(
        LocalDate::new(1900, 2, 29),
        Err(CivilTimeRefusal::InvalidDate)
    );
    assert!(LocalDate::new(2000, 2, 29).is_ok());
    assert!(LocalTime::new(23, 59, 59, 999_999_999).is_ok());
    assert_eq!(
        LocalTime::new(24, 0, 0, 0),
        Err(CivilTimeRefusal::InvalidTime)
    );
}

#[test]
fn offset_floating_and_named_zone_bases_remain_distinct() {
    let offset = UtcOffsetSeconds::new(-8 * 60 * 60).unwrap();
    let zone = NamedTimeZone::new("America/Los_Angeles".into(), "tzdb-2026b".into()).unwrap();
    assert_ne!(CivilTimeBasis::Offset(offset), CivilTimeBasis::Named(zone));
    assert_ne!(CivilTimeBasis::Floating, CivilTimeBasis::Offset(offset));
    assert_eq!(offset.seconds(), -28_800);
    assert_eq!(
        UtcOffsetSeconds::new(86_400),
        Err(CivilTimeRefusal::InvalidOffset)
    );
}

#[test]
fn dst_gap_and_fold_are_explicit_finite_resolution_results() {
    let zone = NamedTimeZone::new("America/Los_Angeles".into(), "tzdb-2026b".into()).unwrap();
    let fold_local = LocalDateTime::new(
        LocalDate::new(2026, 11, 1).unwrap(),
        LocalTime::new(1, 30, 0, 0).unwrap(),
    );
    let fold = ZonedResolution::Ambiguous {
        local: fold_local,
        zone: zone.clone(),
        earlier: instant(1_793_515_800_000),
        later: instant(1_793_519_400_000),
    };
    assert_eq!(fold.validate(), Ok(()));

    let gap_local = LocalDateTime::new(
        LocalDate::new(2026, 3, 8).unwrap(),
        LocalTime::new(2, 30, 0, 0).unwrap(),
    );
    let gap = ZonedResolution::Nonexistent {
        local: gap_local,
        zone,
        gap_before: instant(1_773_047_999_999),
        gap_after: instant(1_773_048_000_000),
    };
    assert_eq!(gap.validate(), Ok(()));
}

#[test]
fn resolution_refuses_reversed_or_incomparable_candidates() {
    let zone = NamedTimeZone::new("America/Los_Angeles".into(), "tzdb-2026b".into()).unwrap();
    let local = LocalDateTime::new(
        LocalDate::new(2026, 11, 1).unwrap(),
        LocalTime::new(1, 30, 0, 0).unwrap(),
    );
    let reversed = ZonedResolution::Ambiguous {
        local,
        zone: zone.clone(),
        earlier: instant(2),
        later: instant(1),
    };
    assert_eq!(
        reversed.validate(),
        Err(CivilTimeRefusal::ReversedResolutionInstants)
    );
    let mut other = instant(2);
    other.clock_basis = "boot/monotonic".into();
    let incomparable = ZonedResolution::Ambiguous {
        local,
        zone,
        earlier: instant(1),
        later: other,
    };
    assert_eq!(
        incomparable.validate(),
        Err(CivilTimeRefusal::IncomparableResolutionInstants)
    );
}
