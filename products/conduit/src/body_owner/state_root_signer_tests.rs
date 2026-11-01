use super::*;
use conduit_body::{Body, BodyMembership};
fn fixture() -> (std::path::PathBuf, BodyBiographyEvidence) {
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "execution-custody",
        "test",
    ));
    fs::create_dir_all(&root).unwrap();
    let installation = Installation {
        schema: super::super::super::INSTALL_SCHEMA.into(),
        host_id: "custody-host".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: digest(b"fixture"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: None,
        selected_todo_checkpoint: None,
    };
    write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let body = Body::born(
        "source/custody".into(),
        "checked/custody".into(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let biography = BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Custody".into(),
    )
    .unwrap();
    retain(&root, &biography, None, None).unwrap();
    (root, biography)
}
#[test]
fn explicit_signer_selection_is_preserved_by_owner_transactions_and_missing_key_refuses() {
    let (root, biography) = fixture();
    let body = biography.body_id.as_str();
    assert!(
        crate::durable_host::root_signer::selected(&root, body, "custody-host")
            .unwrap()
            .is_none()
    );
    assert!(establish_root_signer(&root, "body/wrong").is_err());
    assert!(!root.join("body/root-signer-private.json").exists());
    let selected = establish_root_signer(&root, body).unwrap();
    retain(&root, &biography, None, None).unwrap();
    assert!(establish_root_signer(&root, body).unwrap() == selected);
    fs::remove_file(root.join("body/root-signer-private.json")).unwrap();
    assert!(load(&root).is_err());
    assert!(retain(&root, &biography, None, None).is_err());
    assert!(establish_root_signer(&root, body).is_err());
    assert!(!root.join("body/root-signer-private.json").exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn interrupted_selection_journal_replays_exact_private_identity_and_refuses_replacement() {
    let (root, biography) = fixture();
    let body = biography.body_id.as_str();
    let selected = crate::durable_host::root_signer::prepare(&root, body, "custody-host").unwrap();
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        root_signer: Some(selected.clone()),
        root_receipt: None,
        biography: biography.clone(),
        installation: read_installation(&root.join("installation.json")).unwrap(),
        last_execution: None,
        execution_custody: None,
        admissions: None,
        source: None,
        archives: vec![],
    };
    write_json_atomic(&root.join("body/owner-transaction.json"), &transaction).unwrap();
    assert!(!root.join("body/root-signer.json").exists());
    recover(&root).unwrap();
    assert!(
        crate::durable_host::root_signer::selected(&root, body, "custody-host").unwrap()
            == Some(selected)
    );
    let mut stale = transaction;
    stale.root_signer = None;
    write_json_atomic(&root.join("body/owner-transaction.json"), &stale).unwrap();
    assert!(recover(&root).is_err());
    assert!(root.join("body/owner-transaction.json").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn joined_host_cannot_establish_root_and_command_requires_explicit_authorization() {
    let (root, biography) = fixture();
    let mut installation = read_installation(&root.join("installation.json")).unwrap();
    installation.body_state = None;
    let credential = serde_json::json!({"credential_id":"credential/joined", "body_id":biography.body_id,
        "part_id":"part/joined", "host_id":"custody-host", "boot_id":"boot/joined", "issued_at_millis":1});
    let bytes = serde_json::to_vec(&credential).unwrap();
    let path = root.join("body/membership-credential.json");
    fs::write(&path, &bytes).unwrap();
    installation.joined_body_state = Some(
        serde_json::from_value(serde_json::json!({
        "body_id":biography.body_id, "part_id":"part/joined", "credential_sha256":digest(&bytes),
        "credential_path":path.display().to_string()}))
        .unwrap(),
    );
    write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    assert!(read_installation(&root.join("installation.json")).is_ok());
    assert!(establish_root_signer(&root, biography.body_id.as_str())
        .err()
        .unwrap()
        .contains("joined Host"));
    assert!(!root.join("body/root-signer-private.json").exists());
    assert!(crate::durable_host_control::establish_root_signer(
        &root,
        biography.body_id.as_str(),
        false
    )
    .unwrap_err()
    .contains("--authorize-establish"));
    assert!(!root.join("body/root-signer-private.json").exists());
    fs::remove_dir_all(root).unwrap();
}
