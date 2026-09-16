use conduit_core::{kind_id, SignId, TemporalInstant, TemporalScale};
use conduit_human::*;

fn limits() -> ExperienceLimits {
    ExperienceLimits {
        maximum_items: 4,
        maximum_current_items: 4,
        maximum_recent_items: 4,
        maximum_stale_items: 4,
        maximum_historical_items: 4,
        maximum_items_per_domain: 4,
        maximum_model_derived_items: 4,
        maximum_selected_memory_items: 4,
        maximum_source_refs: 2,
        maximum_relationships: 3,
        maximum_item_bytes: 16,
        maximum_encoded_bytes: 32,
        maximum_conflict_alternatives: 2,
        maximum_identity_bytes: 64,
    }
}

fn at(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/experience".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn new_experience(limits: ExperienceLimits) -> CurrentExperience {
    CurrentExperience::new(
        limits,
        at(100),
        ExperienceTemporalPolicy {
            maximum_current_age_ticks: 5,
            maximum_recent_age_ticks: 20,
        },
    )
    .unwrap()
}

fn item(id: &str, domain: ExperienceDomain, source: &str) -> ExperienceItem {
    ExperienceItem {
        id: id.into(),
        domain,
        content_kind: kind_id("experience/fact@1"),
        encoded_content: id.as_bytes().to_vec(),
        origin: ExperienceOrigin::Observation,
        temporal_role: ExperienceTemporalRole::Current,
        availability: ExperienceAvailability::Present,
        certainty: ExperienceCertainty::Certain,
        observed_at: Some(at(100)),
        recorded_at: None,
        sources: vec![ExperienceSourceRef::Sign(SignId::from(source))],
    }
}

#[test]
fn heterogeneous_items_retain_epistemic_facets_and_sources() {
    let mut experience = new_experience(limits());
    let visual = item("door-open", ExperienceDomain::Visual, "sign/camera/7");
    let mut utterance = item(
        "lets-go",
        ExperienceDomain::HumanUtterance,
        "sign/microphone/4",
    );
    utterance.origin = ExperienceOrigin::HumanReported;
    utterance.certainty = ExperienceCertainty::Uncertain;
    let mut self_state = item("can-move", ExperienceDomain::BodyState, "sign/body/3");
    self_state.temporal_role = ExperienceTemporalRole::Recent;
    self_state.observed_at = Some(at(90));

    experience.try_admit(visual).unwrap();
    experience.try_admit(utterance).unwrap();
    experience.try_admit(self_state).unwrap();

    assert_eq!(experience.items().len(), 3);
    assert_eq!(
        experience.items()[1].origin,
        ExperienceOrigin::HumanReported
    );
    assert_eq!(
        experience.items()[1].certainty,
        ExperienceCertainty::Uncertain
    );
    assert_eq!(
        experience.items()[2].temporal_role,
        ExperienceTemporalRole::Recent
    );
    assert_eq!(
        experience.items()[0].sources,
        vec![ExperienceSourceRef::Sign(SignId::from("sign/camera/7"))]
    );
}

#[test]
fn memory_and_imagination_remain_distinct_from_current_observation() {
    let mut experience = new_experience(limits());
    let mut memory = item("yesterday", ExperienceDomain::Recollection, "sign/memory/1");
    memory.origin = ExperienceOrigin::Remembered;
    assert_eq!(
        experience.try_admit(memory).unwrap_err().refusal,
        ExperienceRefusal::InvalidEpistemicCombination
    );

    let mut imagined = item("blue-door", ExperienceDomain::Visual, "sign/imagination/1");
    imagined.origin = ExperienceOrigin::Imagined;
    experience.try_admit(imagined).unwrap();
    assert_eq!(experience.items()[0].origin, ExperienceOrigin::Imagined);
}

#[test]
fn stale_model_missing_and_unavailable_are_not_collapsed() {
    let mut experience = new_experience(limits());
    let mut model = item("mug", ExperienceDomain::Visual, "sign/model/1");
    model.origin = ExperienceOrigin::ModelDerived;
    model.temporal_role = ExperienceTemporalRole::Stale;
    model.observed_at = Some(at(0));
    model.certainty = ExperienceCertainty::Uncertain;
    model.sources = vec![ExperienceSourceRef::ImplementationRun {
        implementation_id: conduit_core::BaseImplementationId::from("implementation/model@1"),
        provider_instance_id: conduit_core::BaseInstanceId::from("provider/model/boot-1"),
        artifact_id: conduit_core::ArtifactId::from("artifact/model@sha256:abcd"),
        run_id: "run/vision/1".into(),
    }];
    let mut missing = item("hearing", ExperienceDomain::Auditory, "source/microphone");
    missing.availability = ExperienceAvailability::NotObserved;
    missing.encoded_content.clear();
    missing.sources = vec![ExperienceSourceRef::Source {
        source_id: "source/microphone".into(),
    }];
    let mut unavailable = item("location", ExperienceDomain::Location, "source/gps");
    unavailable.availability = ExperienceAvailability::SourceUnavailable;
    unavailable.encoded_content.clear();
    unavailable.sources = vec![ExperienceSourceRef::Source {
        source_id: "source/gps".into(),
    }];

    experience.try_admit(model).unwrap();
    experience.try_admit(missing).unwrap();
    experience.try_admit(unavailable).unwrap();

    assert_eq!(experience.items()[0].origin, ExperienceOrigin::ModelDerived);
    assert_eq!(
        experience.items()[0].temporal_role,
        ExperienceTemporalRole::Stale
    );
    assert_eq!(
        experience.items()[1].availability,
        ExperienceAvailability::NotObserved
    );
    assert_eq!(
        experience.items()[2].availability,
        ExperienceAvailability::SourceUnavailable
    );
}

#[test]
fn contradiction_preserves_both_alternatives_and_exact_provenance() {
    let mut experience = new_experience(limits());
    experience
        .try_admit(item("door-open", ExperienceDomain::Visual, "sign/camera/7"))
        .unwrap();
    experience
        .try_admit(item(
            "door-closed",
            ExperienceDomain::Auditory,
            "sign/human/2",
        ))
        .unwrap();
    experience
        .relate(ExperienceRelation {
            subject_id: "door-open".into(),
            object_id: "door-closed".into(),
            kind: ExperienceRelationKind::Contradicts,
        })
        .unwrap();

    assert_eq!(experience.items().len(), 2);
    assert_eq!(
        experience.relationships()[0].kind,
        ExperienceRelationKind::Contradicts
    );
    assert_ne!(experience.items()[0].sources, experience.items()[1].sources);
}

#[test]
fn source_removal_marks_unavailable_instead_of_inventing_empty_truth() {
    let source = ExperienceSourceRef::Sign(SignId::from("sign/camera/7"));
    let mut experience = new_experience(limits());
    experience
        .try_admit(item("door-open", ExperienceDomain::Visual, "sign/camera/7"))
        .unwrap();
    experience.remove_source(&source);

    assert_eq!(
        experience.items()[0].availability,
        ExperienceAvailability::SourceUnavailable
    );
    assert!(experience.items()[0].sources.is_empty());
    assert!(experience.items()[0].encoded_content.is_empty());
    assert_eq!(experience.encoded_bytes(), 0);
}

#[test]
fn pressure_refuses_without_consuming_the_item_or_mutating_state() {
    let mut bounded = limits();
    bounded.maximum_items = 1;
    bounded.maximum_current_items = 1;
    bounded.maximum_recent_items = 1;
    bounded.maximum_stale_items = 1;
    bounded.maximum_historical_items = 1;
    bounded.maximum_items_per_domain = 1;
    bounded.maximum_model_derived_items = 1;
    bounded.maximum_selected_memory_items = 1;
    let mut experience = new_experience(bounded);
    experience
        .try_admit(item("first", ExperienceDomain::Visual, "sign/1"))
        .unwrap();
    let rejected = item("second", ExperienceDomain::Auditory, "sign/2");
    let error = experience.try_admit(rejected.clone()).unwrap_err();

    assert_eq!(error.refusal, ExperienceRefusal::ItemCapacity);
    assert_eq!(*error.item, rejected);
    assert_eq!(experience.items().len(), 1);
    assert_eq!(experience.encoded_bytes(), 5);
}

#[test]
fn current_experience_contains_no_host_placement_or_effect_authority() {
    let source = include_str!("../src/current_experience.rs");
    for forbidden in [
        "HostId",
        "BootId",
        "AuthorityGrantId",
        "HostOperationId",
        "BodyLifecycleEvent",
        "PresentationAction",
    ] {
        assert!(!source.contains(forbidden));
    }
}

#[test]
fn temporal_domain_model_and_memory_limits_refuse_independently() {
    let mut bounded = limits();
    bounded.maximum_current_items = 1;
    let mut experience = new_experience(bounded);
    experience
        .try_admit(item("visual", ExperienceDomain::Visual, "sign/1"))
        .unwrap();
    assert_eq!(
        experience
            .try_admit(item("audio", ExperienceDomain::Auditory, "sign/2"))
            .unwrap_err()
            .refusal,
        ExperienceRefusal::TemporalRoleCapacity
    );

    bounded = limits();
    bounded.maximum_items_per_domain = 1;
    experience = new_experience(bounded);
    experience
        .try_admit(item("first", ExperienceDomain::Visual, "sign/1"))
        .unwrap();
    let mut second = item("second", ExperienceDomain::Visual, "sign/2");
    second.temporal_role = ExperienceTemporalRole::Recent;
    second.observed_at = Some(at(90));
    assert_eq!(
        experience.try_admit(second).unwrap_err().refusal,
        ExperienceRefusal::DomainCapacity
    );

    bounded = limits();
    bounded.maximum_model_derived_items = 1;
    experience = new_experience(bounded);
    let mut first_model = item("model-1", ExperienceDomain::Visual, "sign/1");
    first_model.origin = ExperienceOrigin::ModelDerived;
    let mut second_model = item("model-2", ExperienceDomain::Auditory, "sign/2");
    second_model.origin = ExperienceOrigin::ModelDerived;
    second_model.temporal_role = ExperienceTemporalRole::Recent;
    second_model.observed_at = Some(at(90));
    experience.try_admit(first_model).unwrap();
    assert_eq!(
        experience.try_admit(second_model).unwrap_err().refusal,
        ExperienceRefusal::ModelDerivedCapacity
    );

    bounded = limits();
    bounded.maximum_selected_memory_items = 1;
    experience = new_experience(bounded);
    let mut first_memory = item("memory-1", ExperienceDomain::Recollection, "sign/1");
    first_memory.origin = ExperienceOrigin::Remembered;
    first_memory.temporal_role = ExperienceTemporalRole::Historical;
    let mut second_memory = item("memory-2", ExperienceDomain::Recollection, "sign/2");
    second_memory.origin = ExperienceOrigin::Remembered;
    second_memory.temporal_role = ExperienceTemporalRole::Historical;
    experience.try_admit(first_memory).unwrap();
    assert_eq!(
        experience.try_admit(second_memory).unwrap_err().refusal,
        ExperienceRefusal::SelectedMemoryCapacity
    );
}

#[test]
fn admission_rejects_a_caller_claimed_temporal_role() {
    let mut experience = new_experience(limits());
    let mut mislabeled = item("old-camera", ExperienceDomain::Visual, "sign/camera/old");
    mislabeled.observed_at = Some(at(70));
    assert_eq!(
        experience.try_admit(mislabeled).unwrap_err().refusal,
        ExperienceRefusal::TemporalRoleMismatch
    );
}
