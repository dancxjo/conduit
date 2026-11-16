//! Process-separated local policy renewal, not remote retirement or MCU Play.
use super::*;
use std::os::unix::fs::PermissionsExt;
const TEST: &str = "durable_host::source_revision::reselection::tests::recovered_claim::real_worker_reselection_keeps_claim_and_allows_return_face";
const PHASE: &str = "CONDUIT_RECOVERY_RESELECT_TEST_PHASE";
const ROOT: &str = "CONDUIT_RECOVERY_RESELECT_TEST_ROOT";

fn phase(root: &Path, which: &str) {
    if which == "reselect" {
        reselect(root, &root.join("renew.json")).unwrap();
        return;
    }
    assert_eq!(which, "resume");
    let installation = read_installation(&root.join("installation.json")).unwrap();
    let selection: Selection =
        serde_json::from_slice(&fs::read(root.join(SELECTION_FILE)).unwrap()).unwrap();
    let prepared = prepare(
        &selection,
        &installation,
        conduit_std_host::StdHostConfig {
            host_id: installation.host_id.as_str().into(),
            boot_id: fresh_identity("boot/recovered-reselection", "fixture").into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        None,
    )
    .unwrap();
    let (host, custody) = prepared.retain_for_restart(&selection).unwrap();
    // These are the ordinary service's two Owner restoration calls. The fresh
    // caller is legitimate local preparation, not reconstructed old authority.
    let mut owner = owner::resume_service(host, root).unwrap();
    owner.restore_selected_file_planning(custody).unwrap();
    assert_eq!(owner.truth()["recovered_source_claim_unresolved"], true);
    assert_eq!(
        owner.selected_source_current_basis().body.state,
        conduit_body::BodyState::Lulled
    );
    assert!(owner
        .plan_retained(root)
        .unwrap_err()
        .contains("recovered-source-claim-unresolved"));
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("viewer-private.json")).unwrap()).unwrap();
    let prior: conduit_body::MembershipCredential =
        serde_json::from_value(saved["credential"].clone()).unwrap();
    let secret: [u8; 32] = serde_json::from_value(saved["secret"].clone()).unwrap();
    let key = conduit_body::SpawnInvitationSecret::from_csprng_bytes(secret).unwrap();
    let mut advertisement: conduit_core::HostAdvertisement =
        serde_json::from_value(saved["advertisement"].clone()).unwrap();
    advertisement.boot_id = "boot/reselected-viewer/2".into();
    let challenge = owner.begin_member_return(&prior, &advertisement).unwrap();
    let proof = conduit_body::PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()),
    };
    let returned = owner.complete_member_return(root, &proof).unwrap();
    assert_eq!(returned.part_id, prior.part_id);
    assert!(owner.local_face_snapshot().is_ok());
    assert!(super::super::super::source_continuation::export(&owner, &[]).is_err());
    assert_eq!(owner.truth()["recovered_source_claim_unresolved"], true);
    assert_eq!(
        owner.selected_source_current_basis().body.workset,
        selection.expected.body.workset
    );
    assert_eq!(
        fs::read(root.join("source-execution/claimed.json")).unwrap(),
        b"unresolved original operation"
    );
}

#[test]
#[ignore = "requires explicit digest-pinned product Source worker artifact"]
fn real_worker_reselection_keeps_claim_and_allows_return_face() {
    if let Some(which) = std::env::var_os(PHASE) {
        phase(
            Path::new(&std::env::var_os(ROOT).unwrap()),
            which.to_str().unwrap(),
        );
        return;
    }
    let (mut owner, root, source_path) = super::super::super::real_owner_state();
    let invitation = owner.issue_invitation(&root, 60, None).unwrap();
    let key = conduit_body::SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let ad = conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: "host/reselected-viewer".into(),
        boot_id: "boot/reselected-viewer/1".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    })
    .advertisement()
    .clone();
    let request = conduit_body::PortableSpawnAdmissionRequest {
        schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: ad.clone(),
        nonce: invitation.claim.nonce,
        signature: key
            .sign(&invitation.claim.signing_transcript(
                &ad.host_id,
                &ad.boot_id,
                ad.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let credential = owner
        .admit_invited(&root, request, ad.host_id.as_str())
        .unwrap()
        .credential;
    write_json_atomic(
        &root.join("viewer-private.json"),
        &serde_json::json!({"credential":credential,"secret":invitation.secret,"advertisement":ad}),
    )
    .unwrap();
    owner.propose_recovered_claim_test_awake(&root).unwrap();
    let awake = owner.selected_source_current_basis();
    assert!(matches!(
        awake.body.state,
        conduit_body::BodyState::Awake { .. }
    ));
    drop(owner);
    let directory = root.join("source-execution");
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let claim = directory.join("claimed.json");
    fs::write(&claim, b"unresolved original operation").unwrap();
    fs::set_permissions(&claim, fs::Permissions::from_mode(0o600)).unwrap();
    let original = fs::read(root.join(SELECTION_FILE)).unwrap();
    let old: Selection = serde_json::from_slice(&original).unwrap();
    assert!(
        old.plays.is_empty()
            && old.returns.is_empty()
            && old.admissions.is_empty()
            && old.continuation_outputs.is_empty()
    );
    let mut installation = read_installation(&root.join("installation.json")).unwrap();
    let previous = installation.release_bundle_sha256.clone();
    installation.release_bundle_sha256 = digest(b"fixture/new-recovery-release");
    write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    write_json_atomic(
        &root.join("renew.json"),
        &serde_json::json!({
            "original_policy_sha256":digest(&original), "previous_release_bundle_sha256":previous,
            "installed_release_bundle_sha256":installation.release_bundle_sha256,
            "expected":awake, "retire_actions":false,
        }),
    )
    .unwrap();
    // Each child exec has its own process-lifetime CATALOG_CUSTODY, just like
    // stopped policy renewal and the subsequent fresh Root in production.
    for which in ["reselect", "resume"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                TEST,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(PHASE, which)
            .env(ROOT, &root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{which} child failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
    }
    let new: Selection =
        serde_json::from_slice(&fs::read(root.join(SELECTION_FILE)).unwrap()).unwrap();
    let mut expected = serde_json::to_value(&old).unwrap();
    expected["expected"] = serde_json::to_value(&awake).unwrap();
    expected["release_bundle_sha256"] = serde_json::json!(installation.release_bundle_sha256);
    assert_eq!(serde_json::to_value(new).unwrap(), expected);
    assert_eq!(fs::read(&claim).unwrap(), b"unresolved original operation");
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(source_path).unwrap();
}
