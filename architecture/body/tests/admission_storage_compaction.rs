//! Actual retained admission records, including private challenge attempt counts,
//! are moved without altering their wire contents or nested identity storage.
use conduit_body::{AdmissionManager, AdmissionStorageCompactionRefusal};
use serde_json::json;
#[path = "common/admission_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn manager() -> AdmissionManager {
    let pending = json!({
        "challenge": {"admission_id":"admission/pending", "body_id":"body/storage", "candidate_id":"candidate/one", "host_id":"host/one", "boot_id":"boot/one", "offer_generation":1, "nonce":vec![9;32], "issued_at_millis":1, "expires_at_millis":10},
        "verifying_key":vec![7;32], "attempts":2
    });
    let invitation = json!({"invitation_id":"invitation/one", "verifying_key":vec![8;32], "nonce":vec![6;32], "issued_at_millis":1, "expires_at_millis":10, "used":true, "attempts":3});
    let key = json!({"part_id":"part/one", "host_id":"host/one", "verifying_key":vec![7;32]});
    let returned = json!({"challenge":{"admission_id":"admission/return", "body_id":"body/storage", "part_id":"part/one", "host_id":"host/one", "boot_id":"boot/next", "offer_generation":2, "nonce":vec![5;32], "issued_at_millis":2, "expires_at_millis":11}, "attempts":1});
    let receipt = json!({"admission_id":"admission/completed", "credential":{"credential_id":"credential/one", "body_id":"body/storage", "part_id":"part/one", "host_id":"host/one", "boot_id":"boot/one", "issued_at_millis":2}});
    serde_json::from_slice(&serde_json::to_vec(&json!({"body_id":"body/storage", "pending":vec![pending;3], "invitations":vec![invitation;3], "continuity_keys":vec![key;6], "pending_returns":vec![returned;3], "receipts":vec![receipt;17]})).unwrap()).unwrap()
}

fn rows(manager: &AdmissionManager) -> [conduit_body::VecStorageUsage; 5] {
    let usage = manager.storage_usage();
    [
        usage.pending_admissions,
        usage.invitations,
        usage.continuity_keys,
        usage.pending_returns,
        usage.receipts,
    ]
}

#[test]
fn actual_record_compaction_preserves_wire_order_attempts_and_nested_identity_backing() {
    let mut manager = manager();
    let before = serde_json::to_vec(&manager).unwrap();
    let (old, accounting) = allocation_probe::observe(|| rows(&manager));
    assert_eq!(
        accounting.allocations + accounting.reallocations + accounting.deallocations,
        0
    );
    assert!(old.iter().all(|r| r.capacity > r.len));
    let pointer = manager
        .receipts
        .owned(0)
        .unwrap()
        .credential
        .credential_id
        .as_str()
        .as_ptr();
    let (result, observed) = allocation_probe::observe(|| manager.try_compact_storage());
    assert_eq!(result, Ok(()));
    assert_eq!(observed.allocations, 5);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(serde_json::to_vec(&manager).unwrap(), before);
    assert_eq!(
        manager
            .receipts
            .owned(0)
            .unwrap()
            .credential
            .credential_id
            .as_str()
            .as_ptr(),
        pointer
    );
    let after = rows(&manager);
    assert!(after.iter().all(|r| r.capacity == r.len));
    let reclaimed: usize = old
        .iter()
        .zip(after)
        .map(|(a, b)| (a.capacity - b.capacity) * a.element_bytes)
        .sum();
    assert!(reclaimed > 0);
    println!(
        "inline storage before={old:?} after={after:?} reclaimed_bytes={reclaimed} replacement_peak_bytes={}",
        observed.peak_bytes
    );
    assert_eq!(
        observed.deallocated_bytes - observed.requested_bytes,
        reclaimed
    );
    let (result, memory) = allocation_probe::observe(|| manager.try_compact_storage());
    assert_eq!(result, Ok(()));
    assert_eq!(
        memory.allocations + memory.reallocations + memory.deallocations,
        0
    );
}

#[test]
fn each_real_replacement_allocation_can_refuse_atomically() {
    let sizes = rows(&manager()).map(|row| row.len * row.element_bytes);
    for (index, bytes) in sizes.into_iter().enumerate() {
        let mut manager = manager();
        let before = serde_json::to_vec(&manager).unwrap();
        let usage = manager.storage_usage();
        let pointer = manager.receipts.as_ptr();
        let (result, observed) = allocation_probe::observe(|| {
            let _refusal = allocation_probe::refuse_next_allocation_size(bytes);
            manager.try_compact_storage()
        });
        assert_eq!(result, Err(AdmissionStorageCompactionRefusal::Capacity));
        assert_eq!(manager.storage_usage(), usage);
        assert_eq!(manager.receipts.as_ptr(), pointer);
        assert_eq!(serde_json::to_vec(&manager).unwrap(), before);
        assert_eq!(observed.allocations, index);
        assert_eq!(observed.deallocations, index);
        assert_eq!(observed.retained_bytes(), 0);
    }
}

#[test]
fn empty_collections_release_only_spare_backing_without_allocating() {
    let mut manager = manager();
    manager.retire_unfinished_challenges();
    manager.try_compact_storage().unwrap();
    let usage = manager.storage_usage();
    assert_eq!(
        (
            usage.pending_admissions.capacity,
            usage.invitations.capacity,
            usage.pending_returns.capacity
        ),
        (0, 0, 0)
    );
    let mut empty = AdmissionManager::new(manager.body_id.clone()).unwrap();
    empty.receipts.try_reserve_exact(3).unwrap();
    let (result, memory) = allocation_probe::observe(|| empty.try_compact_storage());
    assert_eq!(result, Ok(()));
    assert_eq!(empty.receipts.capacity(), 0);
    assert_eq!(memory.allocations + memory.reallocations, 0);
    assert_eq!(memory.deallocations, 1);
    let (result, memory) = allocation_probe::observe(|| empty.try_compact_storage());
    assert_eq!(result, Ok(()));
    assert_eq!(
        memory.allocations + memory.reallocations + memory.deallocations,
        0
    );
}
