use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};
fn root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
        "recovered-claim",
        "tests",
    ));
    std::fs::create_dir(&root).unwrap();
    root
}
fn marker(root: &Path) {
    let dir = root.join("source-execution");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.join("claimed.json");
    // Content is intentionally invalid: only denial, never parsed authority.
    std::fs::write(&path, b"not a command").unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
#[test]
fn absent_claim_is_normal_and_arbitrary_private_content_only_denies() {
    let root = root();
    assert!(!inspect(&root).unwrap());
    marker(&root);
    assert!(inspect(&root).unwrap());
    assert_eq!(
        std::fs::read(root.join("source-execution/claimed.json")).unwrap(),
        b"not a command"
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn unsafe_marker_and_parent_refuse_instead_of_clearing_hold() {
    let root = root();
    marker(&root);
    let path = root.join("source-execution/claimed.json");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(inspect(&root).is_err());
    std::fs::remove_file(&path).unwrap();
    symlink("missing", &path).unwrap();
    assert!(inspect(&root).is_err());
    std::fs::remove_file(path).unwrap();
    std::fs::set_permissions(
        root.join("source-execution"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert!(inspect(&root).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

fn host(boot: &str) -> conduit_std_host::StdHost {
    conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: "host/recovered-claim-test".into(),
        boot_id: boot.into(),
        offer_generation: conduit_core::OfferGeneration(1),
    })
}
#[test]
fn real_resume_keeps_claim_and_readonly_face_but_denies_planning_and_start() {
    let root = root();
    let installation = crate::durable_host::Installation {
        schema: crate::durable_host::INSTALL_SCHEMA.into(),
        host_id: "host/recovered-claim-test".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: crate::durable_host::digest(b"fixture"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: None,
        selected_todo_checkpoint: None,
    };
    crate::durable_host::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let source = crate::plot_source::parse(
        "plot hello {\n show: face/text(\"Message\")\n \"Hello.\" >> show\n}.",
    )
    .unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    let resident = conduit_body::ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let mut original = Owner::open(host("boot/first"), resident, None, "Recovery fixture").unwrap();
    original.persist(&root).unwrap();
    let invitation = original.issue_invitation(&root, 60, None).unwrap();
    let key = conduit_body::SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let guest = conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: "host/returning-viewer".into(),
        boot_id: "boot/viewer/1".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    })
    .advertisement()
    .clone();
    let request = conduit_body::PortableSpawnAdmissionRequest {
        schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: guest.clone(),
        nonce: invitation.claim.nonce,
        signature: key
            .sign(&invitation.claim.signing_transcript(
                &guest.host_id,
                &guest.boot_id,
                guest.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let prior = original
        .admit_invited(&root, request, "Viewer")
        .unwrap()
        .credential;
    drop(original);
    marker(&root);
    let bytes = std::fs::read(root.join("source-execution/claimed.json")).unwrap();
    let mut resumed = crate::durable_host::owner::resume_service(host("boot/next"), &root).unwrap();
    assert!(resumed.recovered_source_claim);
    let mut fresh = guest;
    fresh.boot_id = "boot/viewer/2".into();
    let challenge = resumed.begin_member_return(&prior, &fresh).unwrap();
    let proof = conduit_body::PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()),
    };
    let returned = resumed.complete_member_return(&root, &proof).unwrap();
    assert_eq!(returned.part_id, prior.part_id);
    assert!(resumed.recovered_source_claim);
    assert!(resumed
        .local_face_snapshot_without_wake_input_readiness()
        .is_ok());
    assert!(resumed
        .plan(&plot)
        .unwrap_err()
        .contains("recovered-source-claim-unresolved"));
    assert!(resumed.ensure_execution_retained(&root).is_err());
    assert!(resumed.require_remote_workset_retired().is_err());
    let member = crate::durable_host::owner::MemberPresentationBasis::new(returned, fresh).unwrap();
    let challenge = conduit_body::HostCommandChallenge {
        body_id: "invalid",
        part_id: "invalid",
        credential_id: "invalid",
        receiver_host: "invalid",
        receiver_boot: "invalid",
        receiver_generation: 0,
        receiver_profile: "invalid",
        issuer_host: "invalid",
        issuer_boot: "invalid",
        issuer_generation: 0,
        issuer_key: [0; 32],
        payload_contract: "invalid",
        maximum_payload_bytes: 0,
        nonce: [0; 32],
        issued_at_millis: 0,
        expires_at_millis: 0,
    };
    let limits = conduit_embedded_build::BodyFragmentProjectionLimits {
        image: conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
        computations: conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: 1,
            maximum_total_program_bytes: 1,
        },
        assigned: conduit_core::AssignedPlanMaxima::SINGLE_SOURCE,
        maximum_fore_ports: 1,
        maximum_document_bytes: 1,
    };
    // Denial precedes normal approval validation/signing, even for an invalid
    // challenge: no issuer, projection or remote grant can be manufactured.
    assert!(resumed
        .approve_remote_workset(&member, &challenge, &[], &[], limits)
        .err()
        .unwrap()
        .contains("recovered-source-claim-unresolved"));

    assert!(
        crate::durable_host::source_continuation::export(&resumed, &[])
            .unwrap_err()
            .contains("recovered-source-claim-unresolved")
    );
    assert!(resumed.session.realization().is_none());
    assert_eq!(
        std::fs::read(root.join("source-execution/claimed.json")).unwrap(),
        bytes
    );
    // File removal cannot silently release an already recovered denial.
    std::fs::remove_file(root.join("source-execution/claimed.json")).unwrap();
    resumed.hold_recovered_source_claim(false);
    assert!(resumed.require_source_effects_unheld().is_err());
    resumed.persist(&root).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires explicit digest-pinned product Source worker artifact"]
fn real_image_file_custody_restores_through_standard_runtime_under_hold() {
    let (host, custody, biography, source_path) =
        crate::durable_host::source_revision::real_owner_custody();
    let root = root();
    let path = root.join("original-biography.json");
    let bytes = serde_json::to_vec(&biography).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let installation = crate::durable_host::Installation {
        schema: crate::durable_host::INSTALL_SCHEMA.into(),
        host_id: host.advertisement().host_id.as_str().into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: crate::durable_host::digest(b"fixture"),
        product_executable: "fixture-unused".into(),
        body_state: Some(crate::durable_host::BodyBinding {
            body_id: biography.body_id.as_str().into(),
            biography_sha256: crate::durable_host::digest(&bytes),
            biography_path: path.to_str().unwrap().into(),
        }),
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: None,
        selected_todo_checkpoint: None,
    };
    crate::durable_host::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    marker(&root);
    let mut runtime = crate::durable_host_control::DurableHostRuntime::new(
        "fixture".into(),
        "fixture".into(),
        host,
    );
    runtime.retain_source_preparation(Some(custody));
    // Actual normal service owner restoration, including original IMAGE/File
    // custody: no adoption, replacement, peer IO or original command replay.
    let runtime = runtime.with_owned_body(&root).unwrap();
    assert!(inspect(&root).unwrap());
    drop(runtime);
    assert_eq!(
        std::fs::read(root.join("source-execution/claimed.json")).unwrap(),
        b"not a command"
    );
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_file(source_path).unwrap();
}

#[test]
fn held_legacy_adoption_and_foreground_refuse_before_source_or_worker_access() {
    let root = root();
    marker(&root);
    let missing = root.join("must-not-open-source-or-policy");
    let refusal = crate::durable_host::source_revision::adopt(&root, &missing, false).unwrap_err();
    assert!(
        refusal.contains("recovered-source-claim-unresolved"),
        "{refusal}"
    );
    let refusal = crate::durable_host::owner::run(&missing, &root, "Unchanged").unwrap_err();
    assert!(
        refusal.contains("recovered-source-claim-unresolved"),
        "{refusal}"
    );
    assert!(!root.join("source-policy-owner-peer.json").exists());
    assert!(!root.join("installation.json").exists());
    assert_eq!(
        std::fs::read(root.join("source-execution/claimed.json")).unwrap(),
        b"not a command"
    );
    std::fs::remove_dir_all(root).unwrap();
}
