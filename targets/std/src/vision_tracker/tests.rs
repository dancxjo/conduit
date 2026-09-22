use super::*;
use conduit_core::{
    kind_id, ArtifactId, BaseImplementationId, BaseInstanceId, BoundedResourceRef, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, SignId,
    TemporalInstant, TemporalScale,
};
use conduit_human::{
    ImageObservationReference, ImageRegion, ObjectObservation, VisualEvidenceClass,
    VisualObservationProvenance,
};

fn image(version: u8) -> ImageObservationReference {
    ImageObservationReference {
        content: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([7; 32]),
            content_profile: kind_id("media/image-rgb8@1"),
            access_class: ResourceClassId::from("conduit.resource/image-content@1"),
            extent: ResourceExtent {
                bytes: 64 * 48 * 3,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([version; 32]),
                expires_at: None,
            },
        },
        width: 64,
        height: 48,
    }
}

fn object(version: u8, sign: &str, x: u16) -> ObjectObservation {
    ObjectObservation {
        source_image: image(version),
        candidate_label: "bright-component".into(),
        region: ImageRegion {
            x,
            y: 4,
            width: 12,
            height: 10,
        },
        confidence_permille: 1_000,
        provenance: VisualObservationProvenance {
            evidence_class: VisualEvidenceClass::DeterministicDerived,
            observation_sign_id: SignId::from(sign),
            observed_at: TemporalInstant {
                ticks: u64::from(version),
                scale: TemporalScale::Microseconds,
                clock_basis: "boot/camera/monotonic".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            implementation_id: BaseImplementationId::from("std/continuous-local-vision@1"),
            provider_instance_id: BaseInstanceId::from("provider/camera"),
            artifact_id: ArtifactId::from("artifact/local-vision@1"),
            run_id: format!("run/{version}"),
        },
    }
}

#[test]
fn overlapping_candidates_keep_only_local_continuity_and_exact_history() {
    let profile = kind_id("media/image-rgb8@1");
    let first = conduit_semantic_catalog::object_observations_value(
        &[object(1, "sign/object-1", 4)],
        &profile,
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let second = conduit_semantic_catalog::object_observations_value(
        &[object(2, "sign/object-2", 6)],
        &profile,
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut tracker = LocalVisionTracker::prepare(
        "host/vision/boot-1".into(),
        "play/example/vision-track".into(),
    )
    .unwrap();

    let first_output = tracker
        .process(&first, 10, "boot/host/monotonic", "run/track-1")
        .unwrap()
        .to_vec();
    let first_tracks = conduit_semantic_catalog::track_observations_from_value(
        &conduit_core::StructuredInfoValue::from_canonical_bytes(&first_output).unwrap(),
        &profile,
    )
    .unwrap();
    assert_eq!(first_tracks.len(), 1);
    assert_eq!(first_tracks[0].continuity_confidence_permille, 0);
    assert_eq!(
        first_tracks[0].contributing_observation_sign_ids,
        [SignId::from("sign/object-1")]
    );

    let second_output = tracker
        .process(&second, 11, "boot/host/monotonic", "run/track-2")
        .unwrap()
        .to_vec();
    let second_tracks = conduit_semantic_catalog::track_observations_from_value(
        &conduit_core::StructuredInfoValue::from_canonical_bytes(&second_output).unwrap(),
        &profile,
    )
    .unwrap();
    assert_eq!(second_tracks[0].track_id, first_tracks[0].track_id);
    assert_eq!(
        second_tracks[0].contributing_observation_sign_ids,
        [SignId::from("sign/object-1"), SignId::from("sign/object-2")]
    );
    assert!(second_tracks[0].continuity_confidence_permille > 0);
    assert_eq!(second_tracks[0].source_image, image(2));
    assert_eq!(second_tracks[0].provenance.observed_at.ticks, 11);
}
