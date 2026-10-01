use conduit_form::rust_binding::NativeRustBinding;
use conduit_human::{
    ExperienceAvailability, ExperienceCertainty, ExperienceDomain, ExperienceOrigin,
    ExperienceRelationKind, ExperienceTemporalPolicy, ExperienceTemporalRefusal,
    ExperienceTemporalRole, HumanMediaKind, ImageRegion, KeymapRefusal, SourceAvailability,
    VisualEvidenceClass, VisualExperienceRelationKind, VisualImpressionDisposition,
};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn image_region_round_trips_exact_full_domain_coordinates() {
    for region in [
        ImageRegion::new(0, 0, 0, 0).unwrap(),
        ImageRegion::new(u16::MAX, u16::MAX, u16::MAX, u16::MAX).unwrap(),
    ] {
        assert_round_trip(region);
    }
}

#[test]
fn experience_temporal_policy_round_trips_and_retains_relational_validation() {
    let policy = ExperienceTemporalPolicy::new(5, 20).unwrap();
    assert_round_trip(policy);
    assert_eq!(*policy.maximum_current_age_ticks(), 5);
    assert_eq!(*policy.maximum_recent_age_ticks(), 20);
    assert_eq!(policy.validate(), Ok(()));
    assert!(ExperienceTemporalPolicy::new(20, 20).is_err());
    assert!(ExperienceTemporalPolicy::new(u64::MAX, 0).is_err());
}

#[test]
fn experience_temporal_refusals_are_native() {
    for refusal in [
        ExperienceTemporalRefusal::InvalidPolicy,
        ExperienceTemporalRefusal::InvalidInstant,
        ExperienceTemporalRefusal::IncomparableClock,
        ExperienceTemporalRefusal::IntervalOverflow,
        ExperienceTemporalRefusal::FutureObservation,
        ExperienceTemporalRefusal::IndeterminateAge,
    ] {
        assert_round_trip(refusal);
    }
}

#[test]
fn keymap_refusals_round_trip_through_the_exact_native_type() {
    for value in [
        KeymapRefusal::UnknownComposeSequence,
        KeymapRefusal::EmptyUnicodeEntry,
        KeymapRefusal::UnicodeEntryOverflow,
        KeymapRefusal::InvalidUnicodeScalar,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn human_media_kinds_round_trip_through_the_exact_native_type() {
    for value in [HumanMediaKind::Camera, HumanMediaKind::Microphone] {
        assert_round_trip(value);
    }
}

#[test]
fn visual_impression_dispositions_round_trip_through_the_exact_native_type() {
    for value in [
        VisualImpressionDisposition::Complete,
        VisualImpressionDisposition::truncated(4_096).unwrap(),
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn experience_classifications_round_trip_through_exact_native_types() {
    for value in [
        ExperienceDomain::Visual,
        ExperienceDomain::Auditory,
        ExperienceDomain::Location,
        ExperienceDomain::BodyState,
        ExperienceDomain::HumanUtterance,
        ExperienceDomain::Recollection,
    ] {
        assert_round_trip(value);
    }
    for value in [
        ExperienceOrigin::Observation,
        ExperienceOrigin::HumanReported,
        ExperienceOrigin::Remembered,
        ExperienceOrigin::ModelDerived,
        ExperienceOrigin::Imagined,
    ] {
        assert_round_trip(value);
    }
    for value in [
        ExperienceTemporalRole::Current,
        ExperienceTemporalRole::Recent,
        ExperienceTemporalRole::Stale,
        ExperienceTemporalRole::Historical,
    ] {
        assert_round_trip(value);
    }
    for value in [
        ExperienceAvailability::Present,
        ExperienceAvailability::NotObserved,
        ExperienceAvailability::SourceUnavailable,
    ] {
        assert_round_trip(value);
    }
    for value in [ExperienceCertainty::Certain, ExperienceCertainty::Uncertain] {
        assert_round_trip(value);
    }
    for value in [
        ExperienceRelationKind::Relates,
        ExperienceRelationKind::Contradicts,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn source_and_visual_classifications_round_trip_through_exact_native_types() {
    for value in [
        SourceAvailability::Present,
        SourceAvailability::Missing,
        SourceAvailability::Unavailable,
    ] {
        assert_round_trip(value);
    }
    for value in [
        VisualEvidenceClass::DeterministicDerived,
        VisualEvidenceClass::StatisticalCandidate,
        VisualEvidenceClass::ModelDerived,
    ] {
        assert_round_trip(value);
    }
    for value in [
        VisualExperienceRelationKind::Relates,
        VisualExperienceRelationKind::Supports,
        VisualExperienceRelationKind::Contradicts,
    ] {
        assert_round_trip(value);
    }
}
