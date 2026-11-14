//! Software construction evidence: the payload is deliberately not executable.
//! No packaging, deployment, actual Host, Boot or membership is inferred.
use super::*;
use crate::SporeBinding;
use conduit_body::{AdmissionManager, Body, BodyMembership, SpawnInvitationSecret};
use conduit_host_make::{
    build_host_image, check_host_configuration, parse_host_configuration_conduit,
    verify_host_release_bytes, BuildInputs, HostReleaseBounds, ReleaseTargetProfile,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
const PAYLOAD: &[u8] = b"non-executable prior-Make release fixture";

struct Fixture {
    body: BodyBiographyEvidence,
    claim: SpawnInvitationClaim,
    configuration: CheckedHostConfiguration,
    image: HostImage,
    image_manifest: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let body = Body::born(
            "source/authoring".into(),
            "checked/authoring".into(),
            1,
            "sign/original-birth".into(),
        )
        .unwrap();
        let biography = BodyBiographyEvidence::born(
            body.clone(),
            BodyMembership::new(body.body_id.clone()).unwrap(),
            "Existing fixture Body".into(),
        )
        .unwrap();
        let claim = AdmissionManager::new(body.body_id)
            .unwrap()
            .issue_spawn_invitation(
                SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap(),
                [8; 32],
                10,
                1000,
            )
            .unwrap()
            .claim();
        let configuration = checked("next-native-host");
        let (image, image_manifest) = build_host_image(
            configuration.profile().clone(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            &SporeOutputKind::NativeBundle,
            &BuildInputs {
                source_identity: "source/prior-make".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        Self {
            body: biography,
            claim,
            configuration,
            image,
            image_manifest,
        }
    }
    fn release(&self, change: impl FnOnce(&mut serde_json::Value)) -> VerifiedHostReleaseBytes {
        let payload_digest = format!("sha256:{:x}", Sha256::digest(PAYLOAD));
        let aggregate = format!(
            "sha256:{:x}",
            Sha256::digest(format!(
                "conduit.release/host-bundle-content@1\0fixture.data\0{payload_digest}\n"
            ))
        );
        let m = &self.image.manifest;
        let mut manifest = serde_json::json!({
            "schema":"conduit.release/host-bundle@1","target_id":m.target,"make_package_id":m.make_package_id,"output":"native-bundle",
            "builder_adapter":m.builder_adapter,"deployment_adapter":m.deployment_adapter,"source_identity":m.source_identity,
            "bundle_sha256":aggregate,"files":[{"path":"fixture.data","bytes":PAYLOAD.len(),"sha256":payload_digest,"media_type":"application/octet-stream"}]
        });
        change(&mut manifest);
        verify_host_release_bytes(
            &serde_json::to_vec(&manifest).unwrap(),
            &ReleaseTargetProfile {
                target_id: manifest["target_id"].as_str().unwrap(),
                package_id: manifest["make_package_id"].as_str().unwrap(),
                output: manifest["output"].as_str().unwrap(),
                builder_adapter: manifest["builder_adapter"].as_str().unwrap(),
                deployment_adapter: manifest["deployment_adapter"].as_str().unwrap(),
            },
            manifest["source_identity"].as_str().unwrap(),
            HostReleaseBounds {
                maximum_manifest_bytes: 65536,
                maximum_files: 1,
                maximum_file_bytes: 1024,
                maximum_bundle_bytes: 1024,
            },
            |path| {
                assert_eq!(path, "fixture.data");
                Ok(Cursor::new(PAYLOAD))
            },
            || false,
        )
        .unwrap()
    }
    fn selected<'a>(
        &'a self,
        release: &'a VerifiedHostReleaseBytes,
    ) -> JoiningReleaseSelection<'a> {
        JoiningReleaseSelection {
            body: &self.body,
            invitation: &self.claim,
            configuration: &self.configuration,
            output: SporeOutputKind::NativeBundle,
            image: &self.image,
            image_manifest: &self.image_manifest,
            release,
        }
    }
    fn seal(&self, release: &VerifiedHostReleaseBytes, now: u64) -> Result<BuiltSpore, String> {
        seal_joining_host_release(
            self.selected(release),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            now,
        )
    }
}
fn checked(name: &str) -> CheckedHostConfiguration {
    let source = include_str!("../../../../targets/std/profiles/linux-computer.host.conduit")
        .replace("host linux-computer", &format!("host {name}"));
    check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap()
}

#[test]
fn original_body_birth_and_invitation_bind_the_verified_release_aggregate() {
    let fixture = Fixture::new();
    let before = serde_json::to_vec(&fixture.body).unwrap();
    let release = fixture.release(|_| {});
    let seed = fixture.seal(&release, 20).unwrap();
    assert_eq!(seed.manifest.body_id, fixture.body.body_id.as_str());
    assert_eq!(seed.manifest.source_identity, "sign/original-birth");
    assert_eq!(
        seed.manifest.binding,
        SporeBinding::SelfJoining {
            invitation_id: fixture.claim.invitation_id.as_str().into()
        }
    );
    assert_eq!(
        seed.manifest.host_configuration_id,
        fixture.configuration.configuration_id()
    );
    assert_eq!(seed.manifest.image_id, fixture.image.manifest.image_id);
    assert_eq!(
        seed.manifest.image_content_digest,
        release.image_content_digest()
    );
    assert_ne!(
        seed.manifest.image_content_digest,
        format!("sha256:{:x}", Sha256::digest(PAYLOAD))
    );
    assert_eq!(serde_json::to_vec(&fixture.body).unwrap(), before);
}

#[test]
fn otherwise_verified_release_with_different_source_target_or_make_refuses() {
    let fixture = Fixture::new();
    for (field, value) in [
        ("source_identity", "source/another"),
        ("target_id", "std/aarch64/computer"),
        ("make_package_id", "another-package@1"),
        ("output", "browser-bundle"),
        ("builder_adapter", "another-builder@1"),
        ("deployment_adapter", "another-installer@1"),
    ] {
        let release = fixture.release(|manifest| manifest[field] = value.into());
        assert!(
            fixture
                .seal(&release, 20)
                .unwrap_err()
                .contains("selected Make IMAGE/source"),
            "{field}"
        );
    }
}

#[test]
fn expired_invitation_and_wrong_checked_configuration_refuse() {
    let fixture = Fixture::new();
    let release = fixture.release(|_| {});
    assert!(fixture
        .seal(&release, 1000)
        .unwrap_err()
        .contains("Expired"));
    let other = checked("different-native-host");
    let mut selection = fixture.selected(&release);
    selection.configuration = &other;
    assert!(seal_joining_host_release(
        selection,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        20
    )
    .is_err());
}
