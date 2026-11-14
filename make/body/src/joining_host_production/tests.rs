use super::*;
use conduit_body::{AdmissionManager, Body, BodyMembership, SpawnInvitationSecret};
use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};
use conduit_host_make::{build_host_image, BuildInputs};
use std::io::Cursor;

fn body(seed: &str) -> BodyBiographyEvidence {
    let body = Body::born(
        SourceDocumentId::from("source/authoring"),
        CheckedPlotId::from("checked/authoring"),
        1,
        SignId::from(seed),
    )
    .unwrap();
    BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Hamm Creek House".into(),
    )
    .unwrap()
}
fn claim(body: &BodyBiographyEvidence) -> SpawnInvitationClaim {
    AdmissionManager::new(body.body_id.clone())
        .unwrap()
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap(),
            [8; 32],
            10,
            1000,
        )
        .unwrap()
        .claim()
}
fn checked(name: &str) -> CheckedHostConfiguration {
    let source = include_str!("../../../../targets/std/profiles/linux-computer.host.conduit")
        .replace("host linux-computer", &format!("host {name}"));
    conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(&source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap()
}

struct Fixture {
    body: BodyBiographyEvidence,
    invitation: SpawnInvitationClaim,
    configuration: CheckedHostConfiguration,
    image: HostImage,
    image_manifest: Vec<u8>,
    digest: String,
}
impl Fixture {
    fn new() -> Self {
        let body = body("sign/original-birth");
        let invitation = claim(&body);
        let configuration = checked("next-host");
        let (image, image_manifest) = build_host_image(
            configuration.profile().clone(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            &SporeOutputKind::NativeBundle,
            &BuildInputs {
                source_identity: "git:reviewed-source".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        Self {
            body,
            invitation,
            configuration,
            image,
            image_manifest,
            digest: format!("sha256:{:x}", Sha256::digest(b"artifact")),
        }
    }
    fn request(&self) -> JoiningProductionRequest<'_> {
        JoiningProductionRequest {
            request_id: "request/actual-play",
            body: &self.body,
            invitation: &self.invitation,
            configuration: &self.configuration,
            output: SporeOutputKind::NativeBundle,
            artifact: ReviewedJoiningArtifact {
                image: &self.image,
                image_manifest: &self.image_manifest,
                artifact_digest: &self.digest,
                artifact_bytes: 8,
                source_identity: "git:reviewed-source",
            },
            bounds: JoiningProductionBounds {
                maximum_artifact_bytes: 1024,
                maximum_manifest_bytes: 65536,
                maximum_receipt_bytes: 65536,
            },
        }
    }
    fn produce(&self, bytes: &[u8]) -> Result<ProducedJoiningHost<Cursor<Vec<u8>>>, String> {
        produce_joining_host(
            self.request(),
            Cursor::new(bytes.to_vec()),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            || 20,
            || false,
        )
    }
}

#[test]
fn production_retains_exact_original_body_seed_artifact_and_receipt() {
    let fixture = Fixture::new();
    let biography_before = serde_json::to_vec(&fixture.body).unwrap();
    let result = fixture.produce(b"artifact").unwrap();
    assert_eq!(result.artifact.position(), 0);
    assert_eq!(result.artifact.into_inner(), b"artifact");
    assert_eq!(result.spore.manifest.body_id, fixture.body.body_id.as_str());
    assert_eq!(result.spore.manifest.source_identity, "sign/original-birth");
    assert_eq!(result.spore.manifest.image_content_digest, fixture.digest);
    assert_eq!(
        result.spore_manifest,
        serde_json::to_vec(&result.spore.manifest).unwrap()
    );
    let receipt: serde_json::Value = serde_json::from_slice(&result.receipt).unwrap();
    assert_eq!(receipt["disposition"], "prepared");
    assert_eq!(receipt["request_id"], "request/actual-play");
    assert_eq!(receipt["artifact_bytes"], 8);
    assert_eq!(receipt["spore_id"], result.spore.manifest.spore_id);
    assert_eq!(serde_json::to_vec(&fixture.body).unwrap(), biography_before);
}

#[test]
fn production_refuses_changed_truncated_and_extra_artifact_bytes() {
    let fixture = Fixture::new();
    for bytes in [b"changed!".as_slice(), b"short", b"artifact-extra"] {
        assert!(fixture.produce(bytes).is_err());
    }
}

#[test]
fn production_rechecks_invitation_after_artifact_io() {
    let fixture = Fixture::new();
    let mut ticks = [20, 1000].into_iter();
    let result = produce_joining_host(
        fixture.request(),
        Cursor::new(b"artifact"),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        || ticks.next().unwrap(),
        || false,
    );
    assert!(result.err().unwrap().contains("Expired"));
}

#[test]
fn production_refuses_cancellation_before_read_and_after_hash() {
    let fixture = Fixture::new();
    for stop_at in [1, 4, 5] {
        let mut calls = 0;
        let result = produce_joining_host(
            fixture.request(),
            Cursor::new(b"artifact"),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            || 20,
            || {
                calls += 1;
                calls >= stop_at
            },
        );
        assert!(result.err().unwrap().contains("cancelled"));
    }
}

#[test]
fn production_refuses_source_configuration_and_manifest_substitution() {
    let fixture = Fixture::new();
    let other = checked("different-host");
    for variant in 0..3 {
        let mut request = fixture.request();
        match variant {
            0 => request.artifact.source_identity = "git:unreviewed",
            1 => request.configuration = &other,
            _ => request.artifact.image_manifest = b"{}",
        }
        assert!(produce_joining_host(
            request,
            Cursor::new(b"artifact"),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            || 20,
            || false
        )
        .is_err());
    }
}

#[test]
fn production_enforces_artifact_manifest_and_retained_receipt_bounds() {
    let fixture = Fixture::new();
    for variant in 0..3 {
        let mut request = fixture.request();
        match variant {
            0 => request.bounds.maximum_artifact_bytes = 7,
            1 => request.bounds.maximum_manifest_bytes = 1,
            _ => request.bounds.maximum_receipt_bytes = 1,
        }
        assert!(produce_joining_host(
            request,
            Cursor::new(b"artifact"),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            || 20,
            || false
        )
        .is_err());
    }
}
