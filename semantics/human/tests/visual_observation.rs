use conduit_core::{
    kind_id, ArtifactId, BaseImplementationId, BaseInstanceId, BoundedResourceRef, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, SignId,
    TemporalInstant, TemporalScale,
};
use conduit_human::{
    ImageObservationReference, ImageRegion, MotionObservation, ObjectObservation, TrackObservation,
    VisibleTextObservation, VisualEvidenceClass, VisualObservationProvenance,
    VisualObservationRefusal, MAXIMUM_TRACK_OBSERVATIONS,
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

fn provenance(class: VisualEvidenceClass, suffix: &str) -> VisualObservationProvenance {
    VisualObservationProvenance {
        evidence_class: class,
        observation_sign_id: SignId::from(format!("sign/{suffix}")),
        observed_at: TemporalInstant {
            ticks: 10,
            scale: TemporalScale::Milliseconds,
            clock_basis: "clock/camera-boot".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
        implementation_id: BaseImplementationId::from("implementation/vision@1"),
        provider_instance_id: BaseInstanceId::from("provider/vision/boot-1"),
        artifact_id: ArtifactId::from("artifact/model-or-code@1"),
        run_id: format!("run/{suffix}"),
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

#[test]
fn object_text_motion_and_track_keep_exact_source_and_distinct_evidence() {
    let profile = kind_id("media/image-rgb8@1");
    let object = ObjectObservation {
        source_image: image(),
        candidate_label: "mug".into(),
        region: region(),
        confidence_permille: 860,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, "object"),
    };
    object.validate(&profile).unwrap();

    let text = VisibleTextObservation {
        source_image: image(),
        text: "TRINITY".into(),
        region: Some(region()),
        confidence_permille: 910,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, "ocr"),
    };
    text.validate(&profile).unwrap();

    let motion = MotionObservation {
        source_image: image(),
        changed_region: region(),
        change_permille: 320,
        provenance: provenance(VisualEvidenceClass::DeterministicDerived, "change"),
    };
    motion.validate(&profile).unwrap();

    let track = TrackObservation {
        source_image: image(),
        tracking_context_id: "tracking/play-7".into(),
        track_id: "track/local-3".into(),
        contributing_observation_sign_ids: vec![
            object.provenance.observation_sign_id.clone(),
            SignId::from("sign/object/next-frame"),
        ],
        current_region: region(),
        continuity_confidence_permille: 740,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, "track"),
    };
    track.validate(&profile).unwrap();

    assert_eq!(object.source_image.content.identity.digest(), [7; 32]);
    assert_eq!(text.source_image.content.lifetime.version.digest(), [8; 32]);
    assert_ne!(
        motion.provenance.evidence_class,
        object.provenance.evidence_class
    );
    assert_eq!(track.tracking_context_id, "tracking/play-7");
}

#[test]
fn geometry_confidence_profile_and_evidence_class_refuse_exactly() {
    let profile = kind_id("media/image-rgb8@1");
    let mut object = ObjectObservation {
        source_image: image(),
        candidate_label: "candidate".into(),
        region: region(),
        confidence_permille: 800,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, "object"),
    };
    object.region.x = 600;
    assert_eq!(
        object.validate(&profile),
        Err(VisualObservationRefusal::InvalidRegion)
    );
    object.region = region();
    object.confidence_permille = 1_001;
    assert_eq!(
        object.validate(&profile),
        Err(VisualObservationRefusal::ConfidenceBound)
    );
    object.confidence_permille = 800;
    object.provenance.evidence_class = VisualEvidenceClass::DeterministicDerived;
    assert_eq!(
        object.validate(&profile),
        Err(VisualObservationRefusal::WrongEvidenceClass)
    );
    object.provenance.evidence_class = VisualEvidenceClass::StatisticalCandidate;
    assert_eq!(
        object.validate(&kind_id("media/image-gray8@1")),
        Err(VisualObservationRefusal::WrongImageProfile)
    );
}

#[test]
fn tracking_is_finite_local_and_requires_distinct_contributing_signs() {
    let profile = kind_id("media/image-rgb8@1");
    let mut track = TrackObservation {
        source_image: image(),
        tracking_context_id: "tracking/play-7".into(),
        track_id: "track/local-3".into(),
        contributing_observation_sign_ids: vec![SignId::from("sign/a"), SignId::from("sign/a")],
        current_region: region(),
        continuity_confidence_permille: 700,
        provenance: provenance(VisualEvidenceClass::StatisticalCandidate, "track"),
    };
    assert_eq!(
        track.validate(&profile),
        Err(VisualObservationRefusal::DuplicateObservationRef)
    );
    track.contributing_observation_sign_ids = (0..=MAXIMUM_TRACK_OBSERVATIONS)
        .map(|index| SignId::from(format!("sign/{index}")))
        .collect();
    assert_eq!(
        track.validate(&profile),
        Err(VisualObservationRefusal::ObservationRefBound)
    );
}

#[test]
fn observation_records_contain_no_action_or_acquisition_authority() {
    let source = include_str!("../src/visual_observation.rs");
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
