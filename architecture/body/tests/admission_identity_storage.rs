//! Sharing decoded immutable IDs never changes admission contents or authority.
use conduit_body::AdmissionManager;
use serde_json::json;
#[path = "common/admission_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn manager(receipts: usize) -> AdmissionManager {
    let body = "a".repeat(64);
    let keys: Vec<_> = (0..6).map(|i| json!({"part_id":format!("{i:064x}"),"host_id":format!("host/{i}"),"verifying_key":vec![7;32]})).collect();
    let receipts: Vec<_> = (0..receipts).map(|i| json!({"admission_id":format!("{:064x}",i+100),"credential":{"credential_id":format!("{:064x}",i+200),"body_id":body,"part_id":format!("{:064x}",i%6),"host_id":format!("host/{}",i%6),"boot_id":format!("boot/{i}"),"issued_at_millis":i}})).collect();
    serde_json::from_value(json!({"body_id":body,"pending":[],"invitations":[],"continuity_keys":keys,"pending_returns":[],"receipts":receipts})).unwrap()
}
#[test]
fn decoded_twenty_two_receipts_keep_exact_wire_and_release_duplicate_identity_allocations() {
    let mut manager = manager(22);
    let original = serde_json::to_vec(&manager).unwrap();
    let canonical_body = manager.body_id.as_str().as_ptr();
    assert_ne!(
        manager
            .receipts
            .owned(0)
            .unwrap()
            .credential
            .body_id
            .as_str()
            .as_ptr(),
        canonical_body
    );
    let (_, memory) = allocation_probe::observe(|| manager.share_equal_identity_storage());
    assert_eq!(memory.allocations + memory.reallocations, 0);
    assert!(memory.deallocated_bytes >= 2048);
    assert_eq!(serde_json::to_vec(&manager).unwrap(), original);
    for receipt in (0..manager.receipts.len()).map(|i| manager.receipts.owned(i).unwrap()) {
        assert_eq!(receipt.credential.body_id.as_str().as_ptr(), canonical_body);
    }
    for i in 6..manager.receipts.len() {
        assert_eq!(
            manager
                .receipts
                .owned(i)
                .unwrap()
                .credential
                .part_id
                .as_str()
                .as_ptr(),
            manager
                .receipts
                .owned(i % 6)
                .unwrap()
                .credential
                .part_id
                .as_str()
                .as_ptr()
        );
    }
    let (_, repeated) = allocation_probe::observe(|| manager.share_equal_identity_storage());
    assert_eq!(
        repeated.allocations + repeated.reallocations + repeated.deallocations,
        0
    );
    println!("decoded_manager_identity_storage_cpu64: receipts=22 continuity_keys=6 freed_allocations={} freed_bytes={} allocations={} wire_unchanged=true target_saving_unmeasured=true", memory.deallocations,memory.deallocated_bytes,memory.allocations);
}
#[test]
fn foreign_body_and_distinct_ids_are_preserved_including_original_backing() {
    let mut value = serde_json::to_value(manager(4)).unwrap();
    value["receipts"][1]["credential"]["body_id"] = json!("foreign/body");
    value["receipts"][2]["credential"]["part_id"] = json!("distinct/part");
    let mut manager: AdmissionManager = serde_json::from_value(value).unwrap();
    let original = serde_json::to_vec(&manager).unwrap();
    let foreign_body = manager
        .receipts
        .owned(1)
        .unwrap()
        .credential
        .body_id
        .as_str()
        .as_ptr();
    let foreign_part = manager
        .receipts
        .owned(1)
        .unwrap()
        .credential
        .part_id
        .as_str()
        .as_ptr();
    let distinct_part = manager
        .receipts
        .owned(2)
        .unwrap()
        .credential
        .part_id
        .as_str()
        .as_ptr();
    let credential = manager
        .receipts
        .owned(0)
        .unwrap()
        .credential
        .credential_id
        .as_str()
        .as_ptr();
    manager.share_equal_identity_storage();
    assert_eq!(serde_json::to_vec(&manager).unwrap(), original);
    assert_eq!(
        manager
            .receipts
            .owned(1)
            .unwrap()
            .credential
            .body_id
            .as_str()
            .as_ptr(),
        foreign_body
    );
    assert_eq!(
        manager
            .receipts
            .owned(1)
            .unwrap()
            .credential
            .part_id
            .as_str()
            .as_ptr(),
        foreign_part
    );
    assert_eq!(
        manager
            .receipts
            .owned(2)
            .unwrap()
            .credential
            .part_id
            .as_str()
            .as_ptr(),
        distinct_part
    );
    assert_eq!(
        manager
            .receipts
            .owned(0)
            .unwrap()
            .credential
            .credential_id
            .as_str()
            .as_ptr(),
        credential
    );
}
#[test]
fn oversized_decoded_collections_are_left_to_ordinary_validation() {
    let mut manager = manager(33);
    let original = serde_json::to_vec(&manager).unwrap();
    let (_, memory) = allocation_probe::observe(|| manager.share_equal_identity_storage());
    assert_eq!(
        memory.allocations + memory.reallocations + memory.deallocations,
        0
    );
    assert_eq!(serde_json::to_vec(&manager).unwrap(), original);
}

#[test]
fn pending_challenge_attempts_and_return_evidence_keep_exact_wire() {
    let mut value = serde_json::to_value(manager(2)).unwrap();
    let body = value["body_id"].clone();
    value["pending"] = json!([{"challenge":{"admission_id":"pending/one","body_id":body,"candidate_id":"candidate/one","host_id":"host/0","boot_id":"boot/next","offer_generation":3,"nonce":vec![8;32],"issued_at_millis":1,"expires_at_millis":20},"verifying_key":vec![7;32],"attempts":2}]);
    value["pending_returns"] = json!([{"challenge":{"admission_id":"return/one","body_id":body,"part_id":format!("{:064x}",0),"host_id":"host/0","boot_id":"boot/next","offer_generation":3,"nonce":vec![9;32],"issued_at_millis":1,"expires_at_millis":20},"attempts":1}]);
    let mut manager: AdmissionManager = serde_json::from_value(value).unwrap();
    let before = serde_json::to_vec(&manager).unwrap();
    let (_, memory) = allocation_probe::observe(|| manager.share_equal_identity_storage());
    assert_eq!(memory.allocations + memory.reallocations, 0);
    assert!(memory.deallocated_bytes > 0);
    assert_eq!(serde_json::to_vec(&manager).unwrap(), before);
}
