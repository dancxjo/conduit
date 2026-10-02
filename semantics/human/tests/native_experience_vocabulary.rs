use conduit_human::{
    CurrentExperienceInspectionError, ExperienceAvailability, ExperienceCertainty,
    ExperienceDomain, ExperienceOrigin, ExperienceRefusal, ExperienceRelationKind,
    ExperienceSourceRefusal, ExperienceTemporalPolicy, ExperienceTemporalRefusal,
    ExperienceTemporalRole, ExperienceUpdateRefusal, HumanMediaKind, ImageRegion, KeymapRefusal,
    SourceAvailability, VisualEvidenceClass, VisualExperienceRefusal, VisualExperienceRelationKind,
    VisualImpressionDisposition, VisualImpressionRefusal, VisualObservationRefusal,
};
use conduit_plot::rust_binding::NativeRustBinding;

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

#[test]
fn visual_refusal_families_round_trip_through_exact_native_types() {
    for value in [
        VisualObservationRefusal::InvalidImage,
        VisualObservationRefusal::WrongImageProfile,
        VisualObservationRefusal::InvalidImageDimensions,
        VisualObservationRefusal::ImageTooLarge,
        VisualObservationRefusal::InvalidRegion,
        VisualObservationRefusal::EmptyText,
        VisualObservationRefusal::TextBound,
        VisualObservationRefusal::ConfidenceBound,
        VisualObservationRefusal::EmptyIdentity,
        VisualObservationRefusal::IdentityBound,
        VisualObservationRefusal::InvalidObservationTime,
        VisualObservationRefusal::WrongEvidenceClass,
        VisualObservationRefusal::ObservationRefBound,
        VisualObservationRefusal::DuplicateObservationRef,
    ] {
        assert_round_trip(value);
    }

    for value in [
        VisualImpressionRefusal::InvalidSourceImage,
        VisualImpressionRefusal::EmptyText,
        VisualImpressionRefusal::TextBound,
        VisualImpressionRefusal::EmptyIdentity,
        VisualImpressionRefusal::IdentityBound,
        VisualImpressionRefusal::ObservationRefBound,
        VisualImpressionRefusal::DuplicateObservationRef,
        VisualImpressionRefusal::InvalidTruncation,
        VisualImpressionRefusal::WrongEvidenceClass,
        VisualImpressionRefusal::InvalidProvenance(VisualObservationRefusal::InvalidRegion),
    ] {
        assert_round_trip(value);
    }

    for value in [
        VisualExperienceRefusal::InvalidLimits,
        VisualExperienceRefusal::InvalidSourceImage,
        VisualExperienceRefusal::WrongSourceImage,
        VisualExperienceRefusal::ObservationCapacity,
        VisualExperienceRefusal::ObservationKindCapacity,
        VisualExperienceRefusal::TextCapacity,
        VisualExperienceRefusal::DuplicateObservation,
        VisualExperienceRefusal::InvalidObservation(VisualObservationRefusal::ConfidenceBound),
        VisualExperienceRefusal::InvalidImpression(VisualImpressionRefusal::InvalidTruncation),
        VisualExperienceRefusal::RelationCapacity,
        VisualExperienceRefusal::UnknownRelationEndpoint,
        VisualExperienceRefusal::SelfRelation,
        VisualExperienceRefusal::DuplicateRelation,
        VisualExperienceRefusal::ArithmeticOverflow,
    ] {
        assert_round_trip(value);
    }

    assert!(
        !include_str!("../src/visual_observation.rs").contains("pub enum VisualObservationRefusal")
    );
    assert!(
        !include_str!("../src/visual_impression.rs").contains("pub enum VisualImpressionRefusal")
    );
    assert!(
        !include_str!("../src/visual_experience.rs").contains("pub enum VisualExperienceRefusal")
    );
}

#[test]
fn current_experience_refusal_tree_round_trips_through_exact_native_types() {
    for value in [
        ExperienceRefusal::InvalidLimits,
        ExperienceRefusal::EmptyIdentity,
        ExperienceRefusal::DuplicateIdentity,
        ExperienceRefusal::ItemCapacity,
        ExperienceRefusal::TemporalRoleCapacity,
        ExperienceRefusal::DomainCapacity,
        ExperienceRefusal::ModelDerivedCapacity,
        ExperienceRefusal::SelectedMemoryCapacity,
        ExperienceRefusal::ItemBytes,
        ExperienceRefusal::EncodedBytes,
        ExperienceRefusal::SourceCapacity,
        ExperienceRefusal::DuplicateSource,
        ExperienceRefusal::RelationshipCapacity,
        ExperienceRefusal::UnknownRelationshipEndpoint,
        ExperienceRefusal::DuplicateRelationship,
        ExperienceRefusal::ConflictAlternativeCapacity,
        ExperienceRefusal::InvalidEpistemicCombination,
        ExperienceRefusal::IdentityBytes,
        ExperienceRefusal::InvalidSource,
        ExperienceRefusal::InvalidTime,
        ExperienceRefusal::InvalidTemporalContext,
        ExperienceRefusal::TemporalClassification(ExperienceTemporalRefusal::FutureObservation),
        ExperienceRefusal::TemporalRoleMismatch,
        ExperienceRefusal::ArithmeticOverflow,
    ] {
        assert_round_trip(value);
    }

    for value in [
        ExperienceSourceRefusal::InvalidVisualObservation(
            VisualObservationRefusal::WrongEvidenceClass,
        ),
        ExperienceSourceRefusal::InvalidVisualImpression(
            VisualImpressionRefusal::InvalidTruncation,
        ),
        ExperienceSourceRefusal::EmptyValue,
        ExperienceSourceRefusal::ValueBound,
        ExperienceSourceRefusal::InvalidIdentity,
        ExperienceSourceRefusal::InvalidTime,
        ExperienceSourceRefusal::SourceBound,
    ] {
        assert_round_trip(value);
    }

    for value in [
        ExperienceUpdateRefusal::InvalidLimits,
        ExperienceUpdateRefusal::PendingUpdateCapacity,
        ExperienceUpdateRefusal::StaleRevision,
        ExperienceUpdateRefusal::FutureRevision,
        ExperienceUpdateRefusal::RevisionOverflow,
        ExperienceUpdateRefusal::NoSemanticChange,
        ExperienceUpdateRefusal::Experience(ExperienceRefusal::DuplicateIdentity),
    ] {
        assert_round_trip(value);
    }

    for value in [
        CurrentExperienceInspectionError::EmptyItemIdentity,
        CurrentExperienceInspectionError::UnknownItem,
    ] {
        assert_round_trip(value);
    }

    for source in [
        include_str!("../src/current_experience.rs"),
        include_str!("../src/current_experience_trace.rs"),
        include_str!("../src/experience_sources.rs"),
        include_str!("../src/experience_updates.rs"),
    ] {
        assert!(!source.contains("pub enum ExperienceRefusal"));
        assert!(!source.contains("pub enum ExperienceSourceRefusal"));
        assert!(!source.contains("pub enum ExperienceUpdateRefusal"));
        assert!(!source.contains("pub enum CurrentExperienceInspectionError"));
    }
}
