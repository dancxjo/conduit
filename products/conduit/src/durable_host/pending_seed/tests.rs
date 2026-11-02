use super::*;
use crate::native_package_install::LocalNativeInstaller;
use conduit_body::{
    AdmissionManager, Body, BodyBiographyEvidence, BodyMembership, SpawnInvitationSecret,
};
use conduit_host_make::{BodyBoundArtifactIdentity, NativePackageInstaller};
#[path = "../../../../../make/body/tests/common/joining_host_package.rs"]
#[allow(dead_code)]
mod common;

#[test]
fn installed_seed_requests_original_body_from_actual_initialized_boot() {
    let now = current_time_millis().unwrap();
    let (release, spore, invitation, payloads) = common::fixture_at(
        &["conduit-linux-x86_64", "install-linux-x86_64.sh"],
        false,
        now,
    );
    let (owner_manifest, owner_state) = super::super::tests::fixture();
    let mut owner = super::super::install(&owner_manifest, &owner_state).unwrap();
    super::super::start_runtime(&owner_state).unwrap();
    let body = Body::born(
        "source/authoring".into(),
        "checked/authoring".into(),
        0,
        "sign/original-body-birth".into(),
    )
    .unwrap();
    assert_eq!(body.body_id, invitation.claim.body_id);
    let biography = BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Seed owner".into(),
    )
    .unwrap();
    let dir = owner_state.join("body");
    fs::create_dir_all(&dir).unwrap();
    let biography_path = dir.join("biography.json");
    write_json_atomic(&biography_path, &biography).unwrap();
    owner.body_state = Some(BodyBinding {
        body_id: body.body_id.as_str().into(),
        biography_path: biography_path.display().to_string(),
        biography_sha256: digest(&fs::read(&biography_path).unwrap()),
    });
    write_json_atomic(&owner_state.join("installation.json"), &owner).unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let issued = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap(),
            [3; 32],
            now,
            now + 10000,
        )
        .unwrap();
    assert_eq!(issued.claim(), invitation.claim);
    write_json_atomic(&dir.join("admission.json"), &manager).unwrap();

    let mut bytes = Vec::new();
    let receipt = conduit_body_make::write_joining_host_package(
        &release,
        &spore,
        &invitation,
        now + 1,
        |path| Ok(std::io::Cursor::new(payloads[path].clone())),
        &mut bytes,
        || false,
    )
    .unwrap();
    let artifact = BodyBoundArtifactIdentity {
        target_id: spore.manifest.target.clone(),
        image_id: spore.manifest.image_id.clone(),
        image_content_sha256: receipt.image_content_sha256,
        spore_id: spore.manifest.spore_id.clone(),
        artifact_content_sha256: receipt.artifact_content_sha256,
        artifact_bytes: receipt.artifact_bytes,
    };
    let root = owner_state.parent().unwrap().join("seed");
    let source = owner_state.parent().unwrap().join("seed.zip");
    fs::write(&source, bytes).unwrap();
    let mut installer = LocalNativeInstaller::new_without_service_manager(root.clone());
    installer.install_and_start(&source, &artifact).unwrap();
    assert!(has_pending_seed(&root).unwrap());
    assert_ne!(installer.installed_host().unwrap().host_id, owner.host_id);
    assert!(installer.install_and_start(&source, &artifact).is_err());
    assert!(prepare_pending_seed_request(&root, false).is_err());
    let mut alternate = invitation.clone();
    alternate.secret = [91; 32];
    assert!(validate_invitation(&root, &alternate).is_err());
    assert!(validate_completion(&root, "body/foreign").is_err());
    let retained = read(&root).unwrap();
    assert_eq!(retained.invitation.claim, invitation.claim);
    assert_eq!(retained.invitation.secret, invitation.secret);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(root.join(PATH)).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    // Prepare the ordinary installed Host and serve its actual authenticated
    // status handler, not a stand-in advertisement or manually copied IDs.
    let alias = std::env::temp_dir().join(format!(
        "cdseed-{}-{}",
        std::process::id(),
        &digest(root.to_string_lossy().as_bytes())[7..19]
    ));
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    let thread_root = alias.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (_, mut runtime) =
            super::super::runtime_preparation::prepare_runtime(&thread_root).unwrap();
        assert!(runtime.prepare_birth_encounter(&thread_root).is_err());
        crate::durable_host_control::seed_test_client::serve_one(&thread_root, runtime, tx)
    });
    let actual = rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap();
    let request = prepare_pending_seed_request(&alias, true).unwrap();
    server.join().unwrap().unwrap();
    assert_eq!(request.host_advertisement, actual);
    assert!(!request.membership_admitted && !request.plan_created && !request.play_created);
    let receipt =
        super::super::invitation::admit_body_request_document(request, &owner_state, true).unwrap();
    super::super::membership::complete_body_join_document(receipt, &root, true).unwrap();
    assert!(!has_pending_seed(&root).unwrap());
    assert!(!root.join("body/pending-join.json").exists());
    let installation = read_installation(&root.join("installation.json")).unwrap();
    assert_eq!(
        installation.joined_body_state.unwrap().body_id,
        body.body_id.as_str()
    );
    fs::remove_file(alias).unwrap();
    fs::remove_dir_all(owner_state.parent().unwrap()).unwrap();
}

#[test]
fn expired_or_corrupt_seed_never_becomes_a_birth_or_implicit_admission() {
    let (release, _, invitation, _) = common::fixture(&["conduit-linux-x86_64"]);
    let (manifest, state) = super::super::tests::fixture();
    install_pending_seed(&manifest, &state, &invitation, false).unwrap();
    assert!(has_pending_seed(&state).unwrap());
    assert!(prepare_pending_seed_request(&state, true).is_err());
    let (_, mut runtime) = super::super::runtime_preparation::prepare_runtime(&state).unwrap();
    assert!(runtime.prepare_birth_encounter(&state).is_err());
    assert!(read_installation(&state.join("installation.json"))
        .unwrap()
        .joined_body_state
        .is_none());
    fs::write(state.join(PATH), b"corrupt").unwrap();
    assert!(has_pending_seed(&state).is_err());
    assert!(super::super::runtime_preparation::prepare_runtime(&state).is_err());
    drop(release);
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}
