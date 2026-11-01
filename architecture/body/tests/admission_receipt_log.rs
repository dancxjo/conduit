//! Historical representation changes preserve exact receipts and authority inputs.
use conduit_body::*;
use serde_json::json;
#[allow(dead_code)]
#[path = "common/admission_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn manager(count: usize) -> AdmissionManager {
    let receipts:Vec<_>=(0..count).map(|i|json!({"admission_id":format!("{:064x}",i+100),"credential":{"credential_id":format!("{:064x}",i+200),"body_id":"a".repeat(64),"part_id":format!("{:064x}",i%6),"host_id":format!("host/{}",i%6),"boot_id":format!("boot/{i}"),"issued_at_millis":i}})).collect();
    let mut manager:AdmissionManager=serde_json::from_value(json!({"body_id":"a".repeat(64),"pending":[],"invitations":[],"continuity_keys":[],"pending_returns":[],"receipts":receipts})).unwrap();
    manager.share_equal_identity_storage();
    manager
}
#[test]
fn compact_history_keeps_full_legacy_wire_latest_replay_count_and_measured_backing() {
    let mut manager = manager(28);
    let original = serde_json::to_vec(&manager).unwrap();
    let (result, memory) = allocation_probe::observe(|| manager.receipts.try_compact_historical());
    result.unwrap();
    assert_eq!(serde_json::to_vec(&manager).unwrap(), original);
    let original_log: AdmissionManager = serde_json::from_slice(&original).unwrap();
    let original_log = original_log.receipts;
    assert_eq!(manager.receipts, original_log);
    let usage = manager.receipts.storage_usage();
    assert_eq!((usage.owned_receipts, usage.historical_receipts), (6, 22));
    assert!(memory.deallocated_bytes > memory.requested_bytes);
    for receipt in original_log.iter() {
        let id: AdmissionId = serde_json::from_value(json!(match receipt.admission_id {
            ReceiptIdentityRef::Text(v) => v,
            _ => unreachable!(),
        }))
        .unwrap();
        assert!(manager.receipts.contains_admission(&id));
    }
    for index in 22..28 {
        let receipt = original_log.owned(index).unwrap();
        assert_eq!(
            manager.latest_completed_credential(&receipt.credential.part_id),
            Some(&receipt.credential)
        );
    }
    let restored: AdmissionManager = serde_json::from_slice(&original).unwrap();
    assert_eq!(restored, manager);
    let (_, repeat) =
        allocation_probe::observe(|| manager.receipts.try_compact_historical().unwrap());
    assert_eq!(
        repeat.allocations + repeat.reallocations + repeat.deallocations,
        0
    );
    println!("receipt_log_cpu{} rows=28 hot=6 cold=22 allocations={} freed={} allocated={} net_released={} capacities={:?}",usize::BITS,memory.allocations,memory.deallocated_bytes,memory.requested_bytes,memory.deallocated_bytes-memory.requested_bytes,usage.allocation_capacities);
}
#[test]
fn every_compaction_reservation_refuses_without_changing_original_owner_or_wire() {
    let mut compact = manager(28);
    compact.receipts.try_compact_historical().unwrap();
    for size in compact.receipts.storage_usage().allocation_capacities {
        if size == 0 {
            continue;
        }
        let mut manager = manager(28);
        let original = serde_json::to_vec(&manager).unwrap();
        let pointer = manager.receipts.as_ptr();
        let refusal = allocation_probe::refuse_next_allocation_size(size);
        assert_eq!(
            manager.receipts.try_compact_historical(),
            Err(AdmissionStorageCompactionRefusal::Capacity)
        );
        drop(refusal);
        assert_eq!(manager.receipts.as_ptr(), pointer);
        assert_eq!(serde_json::to_vec(&manager).unwrap(), original);
    }
}
#[test]
fn append_recompact_and_quota_preserve_order_latest_and_noncanonical_text() {
    let mut original = manager(28);
    let mut compact = original.clone();
    compact.receipts.try_compact_historical().unwrap();
    for index in 28..MAX_ADMISSION_RECEIPTS {
        let donor = manager(index + 1);
        let receipt = donor.receipts.last().unwrap().clone();
        original.receipts.try_push(receipt.clone()).unwrap();
        compact.receipts.try_push(receipt).unwrap();
        compact.receipts.try_compact_historical().unwrap();
        assert_eq!(compact.receipts.len(), index + 1);
        assert_eq!(compact, original);
        assert_eq!(
            serde_json::to_vec(&compact).unwrap(),
            serde_json::to_vec(&original).unwrap()
        );
    }
    let next = original.receipts.last().unwrap().clone();
    let before = compact.clone();
    assert_eq!(
        compact.receipts.try_push(next),
        Err(AdmissionRefusal::ReceiptCapacityExhausted)
    );
    assert_eq!(compact, before);
    let mut wire = serde_json::to_value(manager(10)).unwrap();
    wire["receipts"][0]["admission_id"] = "legacy/admission".into();
    wire["receipts"][1]["credential"]["credential_id"] = "legacy/credential".into();
    let mut mixed: AdmissionManager = serde_json::from_value(wire).unwrap();
    let before = serde_json::to_vec(&mixed).unwrap();
    mixed.receipts.try_compact_historical().unwrap();
    assert_eq!(serde_json::to_vec(&mixed).unwrap(), before);
    assert!(mixed.receipts.owned(0).is_some() && mixed.receipts.owned(1).is_some());
}
#[test]
fn compact_append_refusal_is_atomic_and_retry_preserves_all_rows() {
    for order_failure in [false, true] {
        let mut manager = manager(28);
        manager.receipts.try_compact_historical().unwrap();
        let mut receipt = manager.receipts.last().unwrap().clone();
        receipt.credential.issued_at_millis += 1;
        let before = serde_json::to_vec(&manager).unwrap();
        let usage = manager.receipts.storage_usage();
        let pointer = manager.receipts.as_ptr();
        let size = if order_failure {
            29
        } else {
            (usage.owned_receipts + 1) * core::mem::size_of::<AdmissionReceipt>()
        };
        let refusal = allocation_probe::refuse_next_allocation_size(size);
        assert_eq!(
            manager.receipts.try_push(receipt.clone()),
            Err(AdmissionRefusal::ReceiptCapacityExhausted)
        );
        drop(refusal);
        assert_eq!(manager.receipts.as_ptr(), pointer);
        assert_eq!(manager.receipts.storage_usage(), usage);
        assert_eq!(serde_json::to_vec(&manager).unwrap(), before);
        manager.receipts.try_push(receipt).unwrap();
        manager.receipts.try_compact_historical().unwrap();
        assert_eq!(manager.receipts.len(), 29);
    }
}
#[path = "admission/receipt_serde_model.rs"]
mod receipt_serde_model;
