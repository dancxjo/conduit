//! File-publication fault fixtures; genuine signed returns are tested by Owner.
use super::*;
use conduit_body::{AdmissionManager, AdmissionReceipt, PartId};

pub(super) fn fixture() -> (std::path::PathBuf, BodyBiographyEvidence, AdmissionManager) {
    let (root, bio) = super::execution_custody_tests::fixture();
    let mut manager = AdmissionManager::new(bio.body_id.clone()).unwrap();
    let part = PartId::bind(&bio.body_id, "storage-fixture", 0).unwrap();
    for index in 0..32 {
        // These are storage rows, not authentication or physical-device proof.
        let receipt: AdmissionReceipt = serde_json::from_value(serde_json::json!({
            "admission_id": format!("admission/storage/{index}"),
            "credential": {
                "credential_id": format!("credential/storage/{index}"),
                "body_id": bio.body_id, "part_id": part,
                "host_id": "host/storage", "boot_id": format!("boot/storage/{index}"),
                "issued_at_millis": index
            }
        }))
        .unwrap();
        manager.receipts.push(receipt);
    }
    retain(&root, &bio, None, Some(&manager)).unwrap();
    (root, bio, manager)
}

fn transaction(
    root: &Path,
    bio: &BodyBiographyEvidence,
    manager: &AdmissionManager,
) -> Transaction {
    let selection = manager.prepare_receipt_retirement().unwrap().unwrap();
    let installation = read_installation(&root.join("installation.json")).unwrap();
    let (next, custody) =
        admission_custody::prepare(root, &installation.host_id, manager, &selection).unwrap();
    Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography: bio.clone(),
        installation,
        last_execution: None,
        execution_custody: None,
        admissions: Some(next),
        admission_custody: Some(custody),
        source: None,
        workset_sources: None,
        archives: Vec::new(),
    }
}

#[test]
fn interrupted_cold_publication_keeps_hot_evidence_and_replays_exactly() {
    let (root, bio, manager) = fixture();
    let transaction = transaction(&root, &bio, &manager);
    let hot = fs::read(root.join("body/admission.json")).unwrap();
    let journal = root.join("body/owner-transaction.json");
    write_json_atomic(&journal, &transaction).unwrap();
    let cold = root.join("body/admission-custody.json");
    fs::create_dir(&cold).unwrap();
    assert!(recover(&root).is_err());
    assert!(journal.is_file());
    assert_eq!(fs::read(root.join("body/admission.json")).unwrap(), hot);
    fs::remove_dir(&cold).unwrap();
    recover(&root).unwrap();
    let next = admissions(&root, &bio.body_id).unwrap().unwrap();
    assert_eq!(next.receipts.len(), 2);
    assert_eq!(next.retired_receipt_count(), 30);
    let exact = fs::read(&cold).unwrap();
    // Lost completion / transaction removal: replay does not duplicate evidence.
    write_json_atomic(&journal, &transaction).unwrap();
    recover(&root).unwrap();
    assert_eq!(fs::read(&cold).unwrap(), exact);
    assert_eq!(admissions(&root, &bio.body_id).unwrap().unwrap(), next);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn interruption_after_cold_commit_recovers_without_releasing_unretained_evidence() {
    let (root, bio, manager) = fixture();
    let transaction = transaction(&root, &bio, &manager);
    let journal = root.join("body/owner-transaction.json");
    write_json_atomic(&journal, &transaction).unwrap();
    admission_custody::commit(
        &root,
        &transaction.installation.host_id,
        transaction.admissions.as_ref().unwrap(),
        transaction.admission_custody.as_ref().unwrap(),
    )
    .unwrap();
    // Cold evidence is durable; hot file is still the original full manager.
    let old: AdmissionManager =
        serde_json::from_slice(&fs::read(root.join("body/admission.json")).unwrap()).unwrap();
    assert_eq!(old, manager);
    recover(&root).unwrap();
    let next = admissions(&root, &bio.body_id).unwrap().unwrap();
    assert_eq!(next.receipts.len(), 2);
    assert_eq!(next.retired_receipt_count(), 30);
    fs::remove_dir_all(root).unwrap();
}
