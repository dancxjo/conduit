use conduit_core::{
    kind_id, ArtifactId, BaseImplementationId, BaseInstanceId, BoundedResourceRef, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, SignId,
    TemporalInstant, TemporalScale,
};
use conduit_human::*;

fn image(generation: u8) -> ImageObservationReference {
    ImageObservationReference {
        content: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([7; 32]),
            content_profile: kind_id("media/image-rgb8@1"),
            access_class: ResourceClassId::from("conduit.resource/image-content@1"),
            extent: ResourceExtent {
                bytes: 640 * 480 * 3,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([generation; 32]),
                expires_at: None,
            },
        },
        width: 640,
        height: 480,
    }
}

fn provenance(class: VisualEvidenceClass, id: &str) -> VisualObservationProvenance {
    VisualObservationProvenance {
        evidence_class: class,
        observation_sign_id: SignId::from(format!("sign/{id}")),
        observed_at: TemporalInstant {
            ticks: 10,
            scale: TemporalScale::Milliseconds,
            clock_basis: "clock/camera-boot".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
        implementation_id: BaseImplementationId::from(format!("implementation/{id}@1")),
        provider_instance_id: BaseInstanceId::from(format!("provider/{id}/boot-1")),
        artifact_id: ArtifactId::from(format!("artifact/{id}@1")),
        run_id: format!("run/{id}"),
    }
}

fn region() -> ImageRegion {
    ImageRegion {
        x: 10,
        y: 20,
        width: 100,
        height: 80,
    }
}

fn limits() -> VisualExperienceLimits {
    VisualExperienceLimits {
        maximum_observations: 8,
        maximum_objects: 2,
        maximum_visible_texts: 2,
        maximum_motions: 2,
        maximum_tracks: 2,
        maximum_impressions: 2,
        maximum_relations: 4,
        maximum_text_bytes: 128,
    }
}

fn object(source: ImageObservationReference, id: &str) -> VisualExperienceObservation {
    VisualExperienceObservation::Object(Box::new(ObjectObservation {
        source_image: source,
        candidate_label: "mug".into(),
        region: region(),
        confidence_permille: 860,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, id),
    }))
}

fn visible_text(source: ImageObservationReference, id: &str) -> VisualExperienceObservation {
    VisualExperienceObservation::VisibleText(Box::new(VisibleTextObservation {
        source_image: source,
        text: "TRINITY".into(),
        region: Some(region()),
        confidence_permille: 910,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, id),
    }))
}

fn motion(source: ImageObservationReference, id: &str) -> VisualExperienceObservation {
    VisualExperienceObservation::Motion(Box::new(MotionObservation {
        source_image: source,
        changed_region: region(),
        change_permille: 320,
        provenance: provenance(VisualEvidenceClass::DeterministicDerived, id),
    }))
}

fn track(source: ImageObservationReference, id: &str) -> VisualExperienceObservation {
    VisualExperienceObservation::Track(Box::new(TrackObservation {
        source_image: source,
        tracking_context_id: "tracking/play-7".into(),
        track_id: "track/local-3".into(),
        contributing_observation_sign_ids: vec![SignId::from("sign/object")],
        current_region: region(),
        continuity_confidence_permille: 740,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, id),
    }))
}

fn impression(source: ImageObservationReference, id: &str) -> VisualExperienceObservation {
    VisualExperienceObservation::Impression(Box::new(VisualImpression {
        source_image: source,
        selected_observation_sign_ids: vec![SignId::from("sign/object")],
        text: "I think I see a church mug".into(),
        model_id: "model/vision-fixture@1".into(),
        prompt_contract_revision: "prompt/visual-description@1".into(),
        disposition: VisualImpressionDisposition::Complete,
        provenance: provenance(VisualEvidenceClass::ModelDerived, id),
    }))
}

#[test]
fn one_visual_experience_relates_typed_evidence_without_collapsing_it() {
    let source = image(8);
    let mut experience =
        VisualExperience::new(source.clone(), kind_id("media/image-rgb8@1"), limits()).unwrap();
    for observation in [
        object(source.clone(), "object"),
        visible_text(source.clone(), "text"),
        motion(source.clone(), "motion"),
        track(source.clone(), "track"),
        impression(source, "impression"),
    ] {
        experience.try_admit(observation).unwrap();
    }
    experience
        .relate(VisualExperienceRelation {
            subject_sign_id: SignId::from("sign/text"),
            object_sign_id: SignId::from("sign/object"),
            kind: VisualExperienceRelationKind::Relates,
        })
        .unwrap();
    experience
        .relate(VisualExperienceRelation {
            subject_sign_id: SignId::from("sign/object"),
            object_sign_id: SignId::from("sign/impression"),
            kind: VisualExperienceRelationKind::Supports,
        })
        .unwrap();

    assert_eq!(experience.observations().len(), 5);
    assert_eq!(experience.relations().len(), 2);
    assert_eq!(
        experience.observations()[2].evidence_class(),
        VisualEvidenceClass::DeterministicDerived
    );
    assert_eq!(
        experience.observations()[4].evidence_class(),
        VisualEvidenceClass::ModelDerived
    );
    assert_eq!(
        experience.source_image().content.lifetime.version.digest(),
        [8; 32]
    );
}

#[test]
fn another_image_generation_refuses_and_returns_exact_observation() {
    let mut experience =
        VisualExperience::new(image(8), kind_id("media/image-rgb8@1"), limits()).unwrap();
    let other = object(image(9), "other-generation");
    let error = experience.try_admit(other.clone()).unwrap_err();

    assert_eq!(error.refusal, VisualExperienceRefusal::WrongSourceImage);
    assert_eq!(*error.observation, other);
    assert!(experience.observations().is_empty());
}

#[test]
fn kind_text_and_relation_pressure_refuse_independently() {
    let source = image(8);
    let mut bounded = limits();
    bounded.maximum_objects = 1;
    let mut experience =
        VisualExperience::new(source.clone(), kind_id("media/image-rgb8@1"), bounded).unwrap();
    experience
        .try_admit(object(source.clone(), "object-1"))
        .unwrap();
    assert_eq!(
        experience
            .try_admit(object(source.clone(), "object-2"))
            .unwrap_err()
            .refusal,
        VisualExperienceRefusal::ObservationKindCapacity
    );

    bounded = limits();
    bounded.maximum_text_bytes = 2;
    experience =
        VisualExperience::new(source.clone(), kind_id("media/image-rgb8@1"), bounded).unwrap();
    assert_eq!(
        experience
            .try_admit(object(source.clone(), "object"))
            .unwrap_err()
            .refusal,
        VisualExperienceRefusal::TextCapacity
    );

    bounded = limits();
    bounded.maximum_relations = 1;
    experience =
        VisualExperience::new(source.clone(), kind_id("media/image-rgb8@1"), bounded).unwrap();
    experience
        .try_admit(object(source.clone(), "object"))
        .unwrap();
    experience
        .try_admit(visible_text(source.clone(), "text"))
        .unwrap();
    experience.try_admit(motion(source, "motion")).unwrap();
    experience
        .relate(VisualExperienceRelation {
            subject_sign_id: SignId::from("sign/text"),
            object_sign_id: SignId::from("sign/object"),
            kind: VisualExperienceRelationKind::Relates,
        })
        .unwrap();
    assert_eq!(
        experience.relate(VisualExperienceRelation {
            subject_sign_id: SignId::from("sign/motion"),
            object_sign_id: SignId::from("sign/object"),
            kind: VisualExperienceRelationKind::Supports,
        }),
        Err(VisualExperienceRefusal::RelationCapacity)
    );
}

#[test]
fn duplicate_signs_and_unrelated_relation_endpoints_refuse() {
    let source = image(8);
    let mut experience =
        VisualExperience::new(source.clone(), kind_id("media/image-rgb8@1"), limits()).unwrap();
    experience
        .try_admit(object(source.clone(), "same"))
        .unwrap();
    assert_eq!(
        experience
            .try_admit(visible_text(source, "same"))
            .unwrap_err()
            .refusal,
        VisualExperienceRefusal::DuplicateObservation
    );
    assert_eq!(
        experience.relate(VisualExperienceRelation {
            subject_sign_id: SignId::from("sign/same"),
            object_sign_id: SignId::from("sign/absent"),
            kind: VisualExperienceRelationKind::Contradicts,
        }),
        Err(VisualExperienceRefusal::UnknownRelationEndpoint)
    );
}

#[test]
fn visual_experience_contains_no_acquisition_presentation_or_effect_authority() {
    let source = include_str!("../src/visual_experience.rs");
    for forbidden in [
        "MediaAcquisition",
        "AuthorityGrantId",
        "HostCallId",
        "PresentationAction",
        "BodyLifecycleEvent",
    ] {
        assert!(!source.contains(forbidden));
    }
}
