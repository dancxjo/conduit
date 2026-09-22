use conduit_core::{
    kind_id, ArtifactId, BaseImplementationId, BaseInstanceId, BoundedResourceRef, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, SignId,
    TemporalInstant, TemporalScale,
};
use conduit_human::{
    ImageObservationReference, VisualEvidenceClass, VisualImpression, VisualImpressionDisposition,
    VisualImpressionRefusal, VisualObservationProvenance, MAXIMUM_IMPRESSION_OBSERVATION_REFS,
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

fn provenance(class: VisualEvidenceClass) -> VisualObservationProvenance {
    VisualObservationProvenance {
        evidence_class: class,
        observation_sign_id: SignId::from("sign/visual-impression/7"),
        observed_at: TemporalInstant {
            ticks: 10,
            scale: TemporalScale::Milliseconds,
            clock_basis: "clock/camera-boot".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 1,
        },
        implementation_id: BaseImplementationId::from("implementation/vision-describe@1"),
        provider_instance_id: BaseInstanceId::from("provider/local-model/boot-2"),
        artifact_id: ArtifactId::from("artifact/model/vision-3@sha256:abcd"),
        run_id: "run/vision-description/9".into(),
    }
}

fn impression() -> VisualImpression {
    VisualImpression {
        source_image: image(),
        selected_observation_sign_ids: vec![
            SignId::from("sign/object/1"),
            SignId::from("sign/ocr/1"),
        ],
        text: "I think I see a mug marked TRINITY.".into(),
        model_id: "model/local-vision/3".into(),
        prompt_contract_revision: "vision/describe-prompt@1".into(),
        disposition: VisualImpressionDisposition::Complete,
        provenance: provenance(VisualEvidenceClass::ModelDerived),
    }
}

#[test]
fn model_impression_keeps_exact_image_context_and_model_run() {
    let value = impression();
    value.validate(&kind_id("media/image-rgb8@1")).unwrap();

    assert_eq!(value.source_image.content.identity.digest(), [7; 32]);
    assert_eq!(
        value.source_image.content.lifetime.version.digest(),
        [8; 32]
    );
    assert_eq!(value.selected_observation_sign_ids.len(), 2);
    assert_eq!(
        value.provenance.evidence_class,
        VisualEvidenceClass::ModelDerived
    );
    assert_eq!(value.model_id, "model/local-vision/3");
    assert_eq!(value.provenance.run_id, "run/vision-description/9");
}

#[test]
fn model_output_cannot_masquerade_as_deterministic_observation() {
    let mut value = impression();
    value.provenance = provenance(VisualEvidenceClass::DeterministicDerived);
    assert_eq!(
        value.validate(&kind_id("media/image-rgb8@1")),
        Err(VisualImpressionRefusal::WrongEvidenceClass)
    );
}

#[test]
fn context_and_truncation_are_bounded_and_exact() {
    let profile = kind_id("media/image-rgb8@1");
    let mut value = impression();
    value.selected_observation_sign_ids = (0..=MAXIMUM_IMPRESSION_OBSERVATION_REFS)
        .map(|index| SignId::from(format!("sign/observation/{index}")))
        .collect();
    assert_eq!(
        value.validate(&profile),
        Err(VisualImpressionRefusal::ObservationRefBound)
    );

    value = impression();
    value.selected_observation_sign_ids = vec![SignId::from("sign/a"), SignId::from("sign/a")];
    assert_eq!(
        value.validate(&profile),
        Err(VisualImpressionRefusal::DuplicateObservationRef)
    );

    value = impression();
    value.disposition = VisualImpressionDisposition::Truncated {
        original_bytes: value.text.len() as u32,
    };
    assert_eq!(
        value.validate(&profile),
        Err(VisualImpressionRefusal::InvalidTruncation)
    );
    value.disposition = VisualImpressionDisposition::Truncated {
        original_bytes: value.text.len() as u32 + 12,
    };
    value.validate(&profile).unwrap();
}

#[test]
fn impression_contains_no_acquisition_presentation_or_action_authority() {
    let source = include_str!("../src/visual_impression.rs");
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
