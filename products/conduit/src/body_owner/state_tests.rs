use super::*;
#[test]
fn checked_source_and_biography_replay_as_one_owner_transaction() {
    use conduit_body::{Body, BodyBiographyEvidence, BodyMembership, ResidentPlot};
    use conduit_core::bind_sign;

    const SOURCE: &str =
        "plot retained-clock {\n clock: time/every(2s)\n clock >> presentation/tick\n}\n";
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "owner-source-journal",
        "checked-clock",
    ));
    fs::create_dir_all(&root).unwrap();
    let installation = Installation {
        schema: super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/source-journal".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_todo_checkpoint: None,
    };
    write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let checked = crate::plot_source::parse(SOURCE)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let body = Body::born(
        checked.expanded.source_document_id.clone(),
        checked.expanded.checked_plot_id.clone(),
        1,
        bind_sign(
            &"host/source-journal".into(),
            &"boot/source-journal".into(),
            None,
            1,
        )
        .sign_id,
    )
    .unwrap();
    let evidence = BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Clock".into(),
    )
    .unwrap();
    let wrong_source = include_bytes!("../../../../plots/clock/main.conduit");
    retain_with_source(&root, &evidence, None, None, Some(SOURCE.as_bytes())).unwrap();
    let committed = read_installation(&root.join("installation.json")).unwrap();
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography: evidence.clone(),
        installation: committed,
        last_execution: None,
        admissions: None,
        source: Some(SOURCE.as_bytes().to_vec()),
        archives: Vec::new(),
    };
    write_json_atomic(&root.join("body/owner-transaction.json"), &transaction).unwrap();
    fs::write(root.join("body/source.conduit"), wrong_source).unwrap();
    fs::write(root.join("body/biography.json"), b"interrupted").unwrap();
    recover(&root).unwrap();
    assert_eq!(
        fs::read(root.join("body/source.conduit")).unwrap(),
        SOURCE.as_bytes()
    );
    assert_eq!(load(&root).unwrap().unwrap(), evidence);
    assert!(!root.join("body/owner-transaction.json").exists());

    let wrong = crate::plot_source::parse(std::str::from_utf8(wrong_source).unwrap())
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    assert_ne!(
        ResidentPlot::new(
            wrong.expanded.source_document_id,
            wrong.expanded.checked_plot_id
        ),
        body.workset.plots()[0]
    );
    assert!(
        retain_with_source(&root, &evidence, None, None, Some(wrong_source))
            .unwrap_err()
            .contains("source differs")
    );
    assert_eq!(load(&root).unwrap().unwrap(), evidence);
    assert_eq!(
        fs::read(root.join("body/source.conduit")).unwrap(),
        SOURCE.as_bytes()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn interrupted_publication_recovers_and_corruption_never_becomes_birth() {
    use conduit_body::{Body, BodyMembership};
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "owner-state-test",
        "transaction",
    ));
    fs::create_dir_all(&root).unwrap();
    let installation = Installation {
        schema: super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/state-test".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_todo_checkpoint: None,
    };
    write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let body = Body::born(
        "source/test".into(),
        "checked/test".into(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let evidence = BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Retained".into(),
    )
    .unwrap();
    let manager = conduit_body::AdmissionManager::new(body.body_id.clone()).unwrap();
    retain(&root, &evidence, None, Some(&manager)).unwrap();
    assert!(root.join("body/admission.json").exists());
    assert!(!root.join("body/owner-admissions.json").exists());
    assert_eq!(
        admissions(&root, &body.body_id).unwrap(),
        Some(manager.clone())
    );
    write_json_atomic(&root.join("body/owner-admissions.json"), &manager).unwrap();
    assert_eq!(
        admissions(&root, &body.body_id).unwrap(),
        Some(manager.clone())
    );
    assert!(
        super::super::super::invitation::issue_body_invitation_document(&root, 60, None)
            .err()
            .unwrap()
            .contains("legacy owner admission authority")
    );
    let mut conflicting = manager.clone();
    conflicting
        .issue_spawn_invitation(
            conduit_body::SpawnInvitationSecret::from_csprng_bytes([13; 32]).unwrap(),
            [17; 32],
            1_000,
            2_000,
        )
        .unwrap();
    write_json_atomic(&root.join("body/owner-admissions.json"), &conflicting).unwrap();
    assert!(admissions(&root, &body.body_id).is_err());
    write_json_atomic(&root.join("body/owner-admissions.json"), &manager).unwrap();
    retain(&root, &evidence, None, Some(&manager)).unwrap();
    assert!(!root.join("body/owner-admissions.json").exists());
    let retained_installation = read_installation(&root.join("installation.json")).unwrap();
    write_json_atomic(
        &root.join("body/owner-transaction.json"),
        &Transaction {
            schema: "conduit.body/owner-transaction@1".into(),
            biography: evidence.clone(),
            installation: retained_installation,
            last_execution: None,
            admissions: Some(manager.clone()),
            source: None,
            archives: Vec::new(),
        },
    )
    .unwrap();
    fs::write(root.join("body/biography.json"), b"interrupted write").unwrap();
    let invited =
        super::super::super::invitation::issue_body_invitation_document(&root, 60, None).unwrap();
    assert_eq!(invited.claim.body_id, body.body_id);
    assert_eq!(load(&root).unwrap().unwrap().body_id, body.body_id);
    assert!(!root.join("body/owner-transaction.json").exists());
    assert_ne!(admissions(&root, &body.body_id).unwrap(), Some(manager));
    fs::write(root.join("body/biography.json"), b"corrupt").unwrap();
    assert!(load(&root).is_err());
    let raw: Installation =
        serde_json::from_slice(&fs::read(root.join("installation.json")).unwrap()).unwrap();
    assert!(raw.body_state.is_some());
    fs::remove_dir_all(root).unwrap();
}
