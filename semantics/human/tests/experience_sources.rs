use conduit_core::{
    kind_id, ArtifactId, BaseImplementationId, BaseInstanceId, BoundedResourceRef, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, SignId,
    TemporalInstant, TemporalScale,
};
use conduit_human::*;

fn instant() -> TemporalInstant {
    TemporalInstant {
        ticks: 50,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/source".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 2,
    }
}

fn image() -> ImageObservationReference {
    ImageObservationReference {
        content: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: kind_id("media/image-rgb8@1"),
            access_class: ResourceClassId::from("conduit.resource/image-content@1"),
            extent: ResourceExtent {
                bytes: 300,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        },
        width: 10,
        height: 10,
    }
}

fn provenance(class: VisualEvidenceClass, sign: &str) -> VisualObservationProvenance {
    VisualObservationProvenance {
        evidence_class: class,
        observation_sign_id: SignId::from(sign),
        observed_at: instant(),
        implementation_id: BaseImplementationId::from("implementation/vision@1"),
        provider_instance_id: BaseInstanceId::from("provider/vision/boot-1"),
        artifact_id: ArtifactId::from("artifact/vision@sha256:abcd"),
        run_id: "run/vision/1".into(),
    }
}

fn limits() -> ExperienceLimits {
    ExperienceLimits {
        maximum_items: 4,
        maximum_source_refs: 8,
        maximum_relationships: 4,
        maximum_item_bytes: 4_096,
        maximum_encoded_bytes: 12_288,
        maximum_conflict_alternatives: 2,
        maximum_identity_bytes: 128,
    }
}

#[test]
fn one_experience_consumes_visual_utterance_and_body_self_sources() {
    let profile = kind_id("media/image-rgb8@1");
    let visual = ObjectObservation {
        source_image: image(),
        candidate_label: "person".into(),
        region: ImageRegion {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        },
        confidence_permille: 820,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, "sign/object/1"),
    };
    let utterance = HumanUtteranceObservation {
        text: "Let's go outside.".into(),
        speaker_id: "human/travis".into(),
        statement_id: "statement/7".into(),
        observation_sign_id: SignId::from("sign/utterance/7"),
        observed_at: instant(),
        certainty: ExperienceCertainty::Certain,
    };
    let self_state = BodySelfObservation {
        state_kind: kind_id("body/mobility-availability@1"),
        canonical_state: vec![1],
        observation_sign_id: SignId::from("sign/body-state/3"),
        observed_at: instant(),
        certainty: ExperienceCertainty::Certain,
    };
    let mut experience = CurrentExperience::new(limits()).unwrap();
    experience
        .try_admit(
            visual_object_experience(
                "experience/person",
                &visual,
                &profile,
                ExperienceTemporalRole::Current,
            )
            .unwrap(),
        )
        .unwrap();
    experience
        .try_admit(
            human_utterance_experience(
                "experience/utterance",
                &utterance,
                ExperienceTemporalRole::Current,
            )
            .unwrap(),
        )
        .unwrap();
    experience
        .try_admit(
            body_self_experience(
                "experience/mobility",
                &self_state,
                ExperienceTemporalRole::Current,
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(experience.items().len(), 3);
    assert_eq!(experience.items()[0].domain, ExperienceDomain::Visual);
    assert_eq!(
        experience.items()[1].origin,
        ExperienceOrigin::HumanReported
    );
    assert_eq!(experience.items()[2].domain, ExperienceDomain::BodyState);
}

#[test]
fn model_impression_remains_model_derived_with_all_selected_sources() {
    let profile = kind_id("media/image-rgb8@1");
    let impression = VisualImpression {
        source_image: image(),
        selected_observation_sign_ids: vec![SignId::from("sign/object/1")],
        text: "I think I see a person near a door.".into(),
        model_id: "model/vision/2".into(),
        prompt_contract_revision: "vision/prompt@1".into(),
        disposition: VisualImpressionDisposition::Complete,
        provenance: provenance(VisualEvidenceClass::ModelDerived, "sign/impression/1"),
    };
    let item = visual_impression_experience(
        "experience/impression",
        &impression,
        &profile,
        ExperienceTemporalRole::Recent,
    )
    .unwrap();

    assert_eq!(item.origin, ExperienceOrigin::ModelDerived);
    assert_eq!(item.certainty, ExperienceCertainty::Uncertain);
    assert_eq!(item.temporal_role, ExperienceTemporalRole::Recent);
    assert!(item
        .sources
        .contains(&ExperienceSourceRef::Sign(SignId::from("sign/object/1"))));
    assert!(item
        .sources
        .contains(&ExperienceSourceRef::ImplementationRun {
            implementation_id: BaseImplementationId::from("implementation/vision@1"),
            provider_instance_id: BaseInstanceId::from("provider/vision/boot-1"),
            artifact_id: ArtifactId::from("artifact/vision@sha256:abcd"),
            run_id: "run/vision/1".into(),
        }));
}

#[test]
fn selected_recollection_keeps_original_sign_historical() {
    let original = ExperienceSourceRef::Sign(SignId::from("sign/battery/yesterday"));
    let record = SelectedRecollection {
        record_id: "memory/record/41".into(),
        content_kind: kind_id("experience/battery-level@1"),
        canonical_content: 41_u16.to_le_bytes().to_vec(),
        occurred_at: instant(),
        recorded_at: TemporalInstant {
            ticks: 70,
            ..instant()
        },
        original_sources: vec![original.clone()],
        certainty: ExperienceCertainty::Certain,
    };
    let item = recollected_experience("experience/remembered-battery", &record).unwrap();

    assert_eq!(item.origin, ExperienceOrigin::Remembered);
    assert_eq!(item.temporal_role, ExperienceTemporalRole::Historical);
    assert_eq!(item.observed_at, Some(instant()));
    assert_eq!(item.recorded_at, Some(record.recorded_at));
    assert!(item.sources.contains(&original));
    assert!(item.sources.contains(&ExperienceSourceRef::MemoryRecord {
        record_id: "memory/record/41".into(),
    }));

    let content_kind = item.content_kind.clone();
    let mut experience = CurrentExperience::new(limits()).unwrap();
    experience.try_admit(item).unwrap();
    assert_eq!(experience.current_observations(&content_kind).count(), 0);
}
