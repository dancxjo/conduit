//! Synthetic payloads with ordinary Make construction and same-Body sealing.
//! No executable target, deployment, Boot or membership claim.
use conduit_body::{
    AdmissionManager, Body, BodyBiographyEvidence, BodyMembership, PortableInvitation,
    SpawnInvitationSecret, INVITATION_SCHEMA,
};
use conduit_body_make::{seal_joining_host_release, BuiltSpore, JoiningReleaseSelection};
use conduit_host_make::{
    build_host_image, check_host_configuration, parse_host_configuration_conduit,
    verify_host_release_bytes, BuildInputs, HostReleaseBounds, ReleaseTargetProfile,
    SporeOutputKind, VerifiedHostReleaseBytes,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Cursor};

pub fn configuration(browser: bool) -> conduit_host_make::CheckedHostConfiguration {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let source = if browser {
        include_str!("../../../../targets/browser/profiles/browser-page.host.conduit")
    } else {
        include_str!("../../../../targets/std/profiles/linux-computer.host.conduit")
    };
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap()
}

pub fn fixture_for(
    paths: &[&str],
    browser: bool,
) -> (
    VerifiedHostReleaseBytes,
    BuiltSpore,
    PortableInvitation,
    BTreeMap<String, Vec<u8>>,
) {
    fixture_at(paths, browser, 0)
}
pub fn fixture_at(
    paths: &[&str],
    browser: bool,
    now: u64,
) -> (
    VerifiedHostReleaseBytes,
    BuiltSpore,
    PortableInvitation,
    BTreeMap<String, Vec<u8>>,
) {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let configuration = configuration(browser);
    let output = if browser {
        SporeOutputKind::BrowserBundle
    } else {
        SporeOutputKind::NativeBundle
    };
    let (image, image_manifest) = build_host_image(
        configuration.profile().clone(),
        &catalog,
        &packages,
        &output,
        &BuildInputs {
            source_identity: "source/prior-reviewed-make".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    let m = &image.manifest;
    let output_name = if browser {
        "browser-bundle"
    } else {
        "native-bundle"
    };
    let payloads: BTreeMap<String, Vec<u8>> = paths
        .iter()
        .map(|path| {
            (
                path.to_string(),
                if *path == "runtime.wasm" {
                    b"\0asm\x01\0\0\0".to_vec()
                } else {
                    format!("reviewed payload {path}").into_bytes()
                },
            )
        })
        .collect();
    let files=payloads.iter().map(|(path,bytes)|serde_json::json!({"path":path,"bytes":bytes.len(),"sha256":format!("sha256:{:x}",Sha256::digest(bytes)),"media_type":if path.starts_with("install") {"text/x-shellscript"}else{"application/vnd.conduit.host+executable"}})).collect::<Vec<_>>();
    let mut aggregate = Sha256::new();
    aggregate.update(b"conduit.release/host-bundle-content@1\0");
    for file in &files {
        aggregate.update(file["path"].as_str().unwrap().as_bytes());
        aggregate.update(b"\0");
        aggregate.update(file["sha256"].as_str().unwrap().as_bytes());
        aggregate.update(b"\n");
    }
    let bundle = format!("sha256:{:x}", aggregate.finalize());
    let manifest = serde_json::json!({"schema":"conduit.release/host-bundle@1","target_id":m.target,"make_package_id":m.make_package_id,"output":output_name,"builder_adapter":m.builder_adapter,"deployment_adapter":m.deployment_adapter,"source_identity":m.source_identity,"bundle_sha256":bundle,"files":files});
    let verified = verify_host_release_bytes(
        &serde_json::to_vec(&manifest).unwrap(),
        &ReleaseTargetProfile {
            target_id: &m.target,
            package_id: &m.make_package_id,
            output: output_name,
            builder_adapter: &m.builder_adapter,
            deployment_adapter: m.deployment_adapter.as_deref().unwrap(),
        },
        &m.source_identity,
        HostReleaseBounds {
            maximum_manifest_bytes: 65536,
            maximum_files: 32,
            maximum_file_bytes: 1024,
            maximum_bundle_bytes: 32768,
        },
        |path| Ok(Cursor::new(payloads[path].clone())),
        || false,
    )
    .unwrap();
    let body = Body::born(
        "source/authoring".into(),
        "checked/authoring".into(),
        0,
        "sign/original-body-birth".into(),
    )
    .unwrap();
    let biography = BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Existing package fixture Body".into(),
    )
    .unwrap();
    let issued = AdmissionManager::new(body.body_id)
        .unwrap()
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap(),
            [3; 32],
            now,
            now + 10000,
        )
        .unwrap();
    let invitation = PortableInvitation {
        schema: INVITATION_SCHEMA.into(),
        claim: issued.claim(),
        secret: issued.secret.copy_for_target_provisioning(),
        rendezvous: None,
    };
    let spore = seal_joining_host_release(
        JoiningReleaseSelection {
            body: &biography,
            invitation: &invitation.claim,
            configuration: &configuration,
            output,
            image: &image,
            image_manifest: &image_manifest,
            release: &verified,
        },
        &catalog,
        &packages,
        now + 1,
    )
    .unwrap();
    assert_eq!(spore.manifest.source_identity, "sign/original-body-birth");
    assert_ne!(
        spore.manifest.source_identity,
        spore.image.manifest.source_identity
    );
    (verified, spore, invitation, payloads)
}
pub fn fixture(
    paths: &[&str],
) -> (
    VerifiedHostReleaseBytes,
    BuiltSpore,
    PortableInvitation,
    BTreeMap<String, Vec<u8>>,
) {
    fixture_for(paths, false)
}
pub fn routed(invitation: &mut PortableInvitation) {
    invitation.schema = conduit_body::ROUTED_INVITATION_SCHEMA.into();
    invitation.rendezvous = Some(conduit_body::SpawnRendezvousDescriptor {
        protocol: 1,
        body_id: invitation.claim.body_id.as_str().into(),
        invitation_id: invitation.claim.invitation_id.as_str().into(),
        candidates: vec![conduit_body::RendezvousCandidate {
            candidate_id: "route/test".into(),
            line_family: conduit_body::RendezvousLineFamily::AuthenticatedConduitLine,
            reachability: "relay/test".into(),
            authentication: conduit_body::RendezvousAuthentication {
                server_identity: "server/test".into(),
                transport_binding_sha256: [22; 32],
            },
            expires_at_millis: 9999,
            maximum_attempts: 2,
            attempt_timeout_millis: 4000,
        }],
    });
}
