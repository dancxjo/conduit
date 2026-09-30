use conduit_form::rust_binding::NativeRustBinding;
use conduit_human::{
    ExperienceAvailability, ExperienceCertainty, ExperienceDomain, ExperienceOrigin,
    ExperienceRelationKind, ExperienceTemporalRole, HumanMediaKind, KeymapRefusal,
    SourceAvailability, VisualEvidenceClass, VisualExperienceRelationKind,
};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
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
