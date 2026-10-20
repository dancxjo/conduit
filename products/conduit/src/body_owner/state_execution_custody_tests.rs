use super::*;
use conduit_body::{Body, BodyMembership};
use serde_json::{json, Value};

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
fn receipt(biography: &BodyBiographyEvidence, ordinal: usize) -> Value {
    json!({"body_id":biography.body_id, "plan_id":format!("plan/{ordinal}"),
        "play":{"active_play_id":format!("play/{ordinal}"),"body_id":biography.body_id},
        "terminal_sign":{"sign_id":format!("sign/{ordinal}"),"active_play_id":format!("play/{ordinal}")},
        "fore_pending_output":{"stage":"adapter-outcome-unknown","sha256":digest(b"old exact output")}})
}
#[test]
fn distinct_receipts_survive_replacement_recovery_and_repeated_metadata_publication() {
    let (root, bio) = fixture();
    let first = receipt(&bio, 1);
    retain(&root, &bio, Some(&first), None).unwrap();
    let next = receipt(&bio, 2);
    retain(&root, &bio, Some(&next), None).unwrap();
    retain(&root, &bio, Some(&next), None).unwrap();
    recover(&root).unwrap();
    assert_eq!(execution(&root).unwrap(), Some(next.clone()));
    assert_eq!(
        execution_history(&root).unwrap()["receipts"],
        json!([first, next])
    );
    assert_eq!(
        execution_history(&root).unwrap()["legacy_prefix_unavailable"],
        false
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn journal_replay_preserves_old_and_new_receipts_after_an_interrupted_archive_write() {
    let (root, bio) = fixture();
    let first = receipt(&bio, 1);
    retain(&root, &bio, Some(&first), None).unwrap();
    let directory = root.join("body/execution-custody");
    let saved = root.join("saved-custody");
    fs::rename(&directory, &saved).unwrap();
    fs::write(&directory, b"unavailable").unwrap();
    let second = receipt(&bio, 2);
    // Preparation must refuse corrupted/missing prior custody before changing state.
    assert!(retain(&root, &bio, Some(&second), None).is_err());
    fs::remove_file(&directory).unwrap();
    fs::rename(&saved, &directory).unwrap();
    let prepared =
        execution_custody::prepare(&root, "custody-host", bio.body_id.as_str(), Some(&second))
            .unwrap();
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography: bio.clone(),
        installation: read_installation(&root.join("installation.json")).unwrap(),
        last_execution: Some(second.clone()),
        execution_custody: Some(prepared),
        admissions: None,
        source: None,
        archives: Vec::new(),
    };
    write_json_atomic(&root.join("body/owner-transaction.json"), &transaction).unwrap();
    // A failure after journaling must keep the complete terminal receipt in the journal.
    fs::rename(&directory, &saved).unwrap();
    fs::write(&directory, b"unavailable").unwrap();
    assert!(recover(&root).is_err());
    assert!(root.join("body/owner-transaction.json").is_file());
    fs::remove_file(&directory).unwrap();
    fs::rename(&saved, &directory).unwrap();
    recover(&root).unwrap();
    recover(&root).unwrap();
    assert_eq!(
        execution_history(&root).unwrap()["receipts"],
        json!([first, second])
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn corruption_cross_body_and_stale_history_refuse_without_erasing_prior_outcomes() {
    let (root, bio) = fixture();
    let first = receipt(&bio, 1);
    retain(&root, &bio, Some(&first), None).unwrap();
    let stale =
        execution_custody::prepare(&root, "custody-host", bio.body_id.as_str(), None).unwrap();
    let second = receipt(&bio, 2);
    retain(&root, &bio, Some(&second), None).unwrap();
    assert!(
        execution_custody::commit(&root, &stale, "custody-host", bio.body_id.as_str()).is_err()
    );
    let wrong = json!({"body_id":"another-body"});
    assert!(retain(&root, &bio, Some(&wrong), None).is_err());
    let index: Value =
        serde_json::from_slice(&fs::read(root.join("body/owner-execution.json")).unwrap()).unwrap();
    let file = root
        .join("body/execution-custody")
        .join(format!("{}.json", index["history"][0].as_str().unwrap()));
    let saved = fs::read(&file).unwrap();
    fs::write(&file, b"corrupt").unwrap();
    assert!(require_execution_capacity(&root).is_err());
    assert!(execution_history(&root).is_err());
    fs::write(file, saved).unwrap();
    assert_eq!(
        execution_history(&root).unwrap()["receipts"],
        json!([first, second])
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn finite_capacity_refuses_before_play_and_never_evicts_old_receipts() {
    let (root, bio) = fixture();
    for n in 0..execution_custody::MAX_RECEIPTS {
        require_execution_capacity(&root).unwrap();
        retain(&root, &bio, Some(&receipt(&bio, n)), None).unwrap();
    }
    let before = fs::read(root.join("body/owner-execution.json")).unwrap();
    assert!(require_execution_capacity(&root)
        .unwrap_err()
        .contains("before Play"));
    assert!(retain(&root, &bio, Some(&receipt(&bio, 999)), None).is_err());
    assert_eq!(
        fs::read(root.join("body/owner-execution.json")).unwrap(),
        before
    );
    assert_eq!(
        execution_history(&root).unwrap()["receipts"]
            .as_array()
            .unwrap()
            .len(),
        execution_custody::MAX_RECEIPTS
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn legacy_latest_migrates_with_an_explicit_unavailable_prefix() {
    let (root, bio) = fixture();
    let first = receipt(&bio, 1);
    write_json_atomic(&root.join("body/owner-execution.json"),&json!({"schema":"conduit.body/owner-execution@1", "host_id":"custody-host", "body_id":bio.body_id,
        "sha256":digest(&serde_json::to_vec(&Some(&first)).unwrap()),"last_execution":first})).unwrap();
    let legacy = execution_history(&root).unwrap();
    assert_eq!(legacy["archive_records_committed"], false);
    assert_eq!(legacy["receipts"], json!([first]));
    let second = receipt(&bio, 2);
    retain(&root, &bio, Some(&second), None).unwrap();
    let history = execution_history(&root).unwrap();
    assert_eq!(history["receipts"], json!([first, second]));
    assert_eq!(history["legacy_prefix_unavailable"], true);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_index_cannot_discard_immutable_outcomes_but_journal_replay_can_complete_it() {
    let (root, bio) = fixture();
    let first = receipt(&bio, 1);
    retain(&root, &bio, Some(&first), None).unwrap();
    let index_path = root.join("body/owner-execution.json");
    let saved = fs::read(&index_path).unwrap();
    let prepared =
        execution_custody::prepare(&root, "custody-host", bio.body_id.as_str(), Some(&first))
            .unwrap();
    fs::remove_file(&index_path).unwrap();
    assert!(require_execution_capacity(&root).is_err());
    assert!(execution(&root).is_err());
    execution_custody::commit(&root, &prepared, "custody-host", bio.body_id.as_str()).unwrap();
    assert_eq!(fs::read(&index_path).unwrap(), saved);
    assert_eq!(
        execution_history(&root).unwrap()["receipts"],
        json!([first])
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn oversized_or_inconsistent_terminal_transaction_refuses_without_replacing_current_receipt() {
    let (root, bio) = fixture();
    let first = receipt(&bio, 1);
    retain(&root, &bio, Some(&first), None).unwrap();
    let before = fs::read(root.join("body/owner-execution.json")).unwrap();
    let oversized = json!({"body_id":bio.body_id,"output":"x".repeat(execution_custody::MAX_RECEIPT_BYTES as usize)});
    assert!(retain(&root, &bio, Some(&oversized), None).is_err());
    let prepared =
        execution_custody::prepare(&root, "custody-host", bio.body_id.as_str(), Some(&first))
            .unwrap();
    let second = receipt(&bio, 2);
    write_json_atomic(
        &root.join("body/owner-transaction.json"),
        &Transaction {
            schema: "conduit.body/owner-transaction@1".into(),
            biography: bio.clone(),
            installation: read_installation(&root.join("installation.json")).unwrap(),
            last_execution: Some(second),
            execution_custody: Some(prepared),
            admissions: None,
            source: None,
            archives: Vec::new(),
        },
    )
    .unwrap();
    assert!(recover(&root).is_err());
    assert_eq!(
        fs::read(root.join("body/owner-execution.json")).unwrap(),
        before
    );
    fs::remove_file(root.join("body/owner-transaction.json")).unwrap();
    assert_eq!(
        execution_history(&root).unwrap()["receipts"],
        json!([first])
    );
    fs::remove_dir_all(root).unwrap();
}
