use conduit_core::{
    kind_id, ArtifactId, BaseImplementationId, BaseInstanceId, BoundedResourceRef, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, SignId,
    StructuredInfoTypeShape, StructuredInfoValueShape, TemporalInstant, TemporalScale,
};
use conduit_human::{
    ImageObservationReference, ImageRegion, ObjectObservation, TrackObservation,
    VisibleTextObservation, VisualEvidenceClass, VisualExperience, VisualExperienceLimits,
    VisualExperienceObservation, VisualExperienceRelation, VisualExperienceRelationKind,
    VisualImpression, VisualImpressionDisposition, VisualObservationProvenance,
};
use conduit_semantic_catalog::{
    object_observations_from_value, object_observations_value, track_observations_from_value,
    track_observations_value, visible_text_observations_from_value,
    visible_text_observations_value, visual_experience_from_value, visual_experience_value,
    visual_impression_from_value, visual_impression_value,
};

fn image() -> ImageObservationReference {
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
                version: ResourceVersionIdentity::from_digest([8; 32]),
                expires_at: None,
            },
        },
        width: 640,
        height: 480,
    }
}

fn provenance(class: VisualEvidenceClass, identity: &str) -> VisualObservationProvenance {
    VisualObservationProvenance {
        evidence_class: class,
        observation_sign_id: SignId::from(format!("sign/{identity}")),
        observed_at: TemporalInstant {
            ticks: 10,
            scale: TemporalScale::Milliseconds,
            clock_basis: "clock/camera-boot".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 1,
        },
        implementation_id: BaseImplementationId::from(format!("implementation/{identity}@1")),
        provider_instance_id: BaseInstanceId::from(format!("provider/{identity}/boot-1")),
        artifact_id: ArtifactId::from(format!("artifact/{identity}@1")),
        run_id: format!("run/{identity}"),
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

fn visible_text() -> VisibleTextObservation {
    VisibleTextObservation {
        source_image: image(),
        text: "TRINITY".into(),
        region: Some(region()),
        confidence_permille: 910,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, "ocr"),
    }
}

fn object() -> ObjectObservation {
    ObjectObservation {
        source_image: image(),
        candidate_label: "bright-component".into(),
        region: region(),
        confidence_permille: 800,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, "object"),
    }
}

fn track() -> TrackObservation {
    TrackObservation {
        source_image: image(),
        tracking_context_id: "tracking/play-7".into(),
        track_id: "track/local-3".into(),
        contributing_observation_sign_ids: vec![SignId::from("sign/object")],
        current_region: region(),
        continuity_confidence_permille: 740,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, "track"),
    }
}

fn impression() -> VisualImpression {
    VisualImpression {
        source_image: image(),
        selected_observation_sign_ids: vec![SignId::from("sign/object")],
        text: "I think I see a church mug".into(),
        model_id: "model/vision-fixture@1".into(),
        prompt_contract_revision: "prompt/visual-description@1".into(),
        disposition: VisualImpressionDisposition::Complete,
        provenance: provenance(VisualEvidenceClass::ModelDerived, "impression"),
    }
}

#[test]
fn observation_values_retain_actual_length_and_exact_domain_truth() {
    let profile = kind_id("media/image-rgb8@1");
    let texts = visible_text_observations_value(&[visible_text()], &profile).unwrap();
    let tracks = track_observations_value(&[track()], &profile).unwrap();
    let objects = object_observations_value(&[object()], &profile).unwrap();
    for value in [&texts, &tracks, &objects] {
        let StructuredInfoValueShape::Collection(values) = value.shape() else {
            panic!("expected bounded sequence value")
        };
        assert_eq!(values.len(), 1);
        assert!(!value.canonical_bytes().unwrap().is_empty());
    }
    let StructuredInfoTypeShape::Sequence { capacity, .. } = texts.value_type().shape() else {
        unreachable!()
    };
    assert_eq!(capacity, 8);
    assert_eq!(
        visible_text_observations_from_value(&texts, &profile).unwrap(),
        [visible_text()]
    );
    assert_eq!(
        track_observations_from_value(&tracks, &profile).unwrap(),
        [track()]
    );
    assert_eq!(
        object_observations_from_value(&objects, &profile).unwrap(),
        [object()]
    );
}

#[test]
fn model_impression_and_experience_encode_without_laundering_evidence() {
    let profile = kind_id("media/image-rgb8@1");
    let impression = impression();
    let encoded_impression = visual_impression_value(&impression, &profile).unwrap();
    assert!(!encoded_impression.canonical_bytes().unwrap().is_empty());
    assert_eq!(
        visual_impression_from_value(&encoded_impression, &profile).unwrap(),
        impression
    );

    let mut experience = VisualExperience::new(
        image(),
        profile,
        VisualExperienceLimits {
            maximum_observations: 2,
            maximum_objects: 1,
            maximum_visible_texts: 1,
            maximum_motions: 1,
            maximum_tracks: 1,
            maximum_impressions: 1,
            maximum_relations: 1,
            maximum_text_bytes: 128,
        },
    )
    .unwrap();
    experience
        .try_admit(VisualExperienceObservation::VisibleText(Box::new(
            visible_text(),
        )))
        .unwrap();
    experience
        .try_admit(VisualExperienceObservation::Impression(Box::new(
            impression,
        )))
        .unwrap();
    experience
        .relate(VisualExperienceRelation {
            subject_sign_id: SignId::from("sign/ocr"),
            object_sign_id: SignId::from("sign/impression"),
            kind: VisualExperienceRelationKind::Supports,
        })
        .unwrap();
    let encoded = visual_experience_value(&experience).unwrap();
    assert!(!encoded.canonical_bytes().unwrap().is_empty());
    assert_eq!(
        visual_experience_from_value(
            &encoded,
            kind_id("media/image-rgb8@1"),
            VisualExperienceLimits {
                maximum_observations: 2,
                maximum_objects: 1,
                maximum_visible_texts: 1,
                maximum_motions: 1,
                maximum_tracks: 1,
                maximum_impressions: 1,
                maximum_relations: 1,
                maximum_text_bytes: 128,
            },
        )
        .unwrap(),
        experience
    );
}
