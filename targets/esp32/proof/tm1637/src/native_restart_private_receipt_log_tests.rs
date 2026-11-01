//! Actual copied checkpoint fixture; no hardware or new custody endorsement.
use super::*;
#[test]
#[ignore = "private pinned actual28-receipt backup; fresh CPU process only"]
fn private_actual_twenty_eight_receipts_compact_and_checkpoint_reopen_exactly() {
    let body: conduit_body::BodyId = serde_json::from_value(serde_json::Value::String(
        std::env::var("CONDUIT_PRIVATE_BODY_ID").unwrap(),
    ))
    .unwrap();
    let root = root_pin();
    let raw = std::fs::read(std::env::var("CONDUIT_PRIVATE_BODY_BACKUP").unwrap()).unwrap();
    assert_eq!(raw.len(), 4194304);
    let binding = Sha256::digest(body.as_str().as_bytes()).into();
    let mut region = crate::native_evidence_region::test_empty();
    region.program(0, &raw[0x3f0000..0x400000]).unwrap();
    let mut journal = Checkpoint::open(region).unwrap();
    let mut seed = PrivateSeed([0; 32]);
    let (mut resident, source) =
        decode_checkpoint(&mut journal, &binding, &body, &mut seed.0, None).unwrap();
    assert!(SigningKey::from_bytes(&seed.0).verifying_key().to_bytes() == root);
    let manager = resident.retained.admissions.as_mut().unwrap();
    assert_eq!(manager.receipts.len(), 28);
    manager.share_equal_identity_storage();
    manager.try_compact_storage().unwrap();
    let original = serde_json::to_vec(&*manager).unwrap();
    let (owned_clone, owned_clone_memory) = crate::allocation_probe::observe(|| manager.clone());
    drop(owned_clone);
    let (result, memory) =
        crate::allocation_probe::observe(|| manager.receipts.try_compact_historical());
    result.unwrap();
    assert_eq!(serde_json::to_vec(&*manager).unwrap(), original);
    let usage = manager.receipts.storage_usage();
    let (compact_clone, compact_clone_memory) =
        crate::allocation_probe::observe(|| manager.clone());
    drop(compact_clone);
    assert!(compact_clone_memory.retained_bytes() <= owned_clone_memory.retained_bytes());
    assert_eq!((usage.owned_receipts, usage.historical_receipts), (6, 22));
    let saving = memory
        .deallocated_bytes
        .checked_sub(memory.requested_bytes)
        .unwrap();
    assert!(
        saving >= 2048,
        "actual CPU fixture must justify target qualification"
    );
    let prefix = serde_json::to_vec(&resident.prefix).unwrap();
    let layouts = serde_json::to_vec(&resident.layouts).unwrap();
    let network = resident.network.as_ref().map(|c| *c.bytes());
    let host: conduit_core::HostId = std::env::var("CONDUIT_PRIVATE_PRODUCER_HOST")
        .unwrap()
        .into();
    let boot: conduit_core::BootId = "boot/copied-real-prefix-join-boot-001".into();
    let mut owner =
        SealedBodyLifecycleSession::resume_here(resident.retained.evidence, &host, &boot).unwrap();
    let mut store = Store {
        journal,
        binding,
        seed: Some(seed.0),
        evidence: None,
        archives: resident.retained.archives,
        custody: Some(ArchiveCustody {
            proof: resident.proof,
            layouts: resident.layouts,
            network: resident.network,
            source,
            summary: resident.summary,
            span: resident.span,
            prefix: resident.prefix,
            local_count: resident.local_count,
        }),
        admissions: resident.retained.admissions,
        parked: false,
        pristine: false,
        failed: false,
        ready: false,
    };
    while owner.seal_terminal_wake_for_checkpoint().unwrap() {
        checkpoint_store(&mut store, &mut owner, &root).unwrap();
    }
    owner.seal_membership_history_for_checkpoint().unwrap();
    checkpoint_store(&mut store, &mut owner, &root).unwrap();
    let (after, _) =
        decode_checkpoint(&mut store.journal, &binding, &body, &mut seed.0, None).unwrap();
    assert!(SigningKey::from_bytes(&seed.0).verifying_key().to_bytes() == root);
    assert_eq!(
        serde_json::to_vec(&after.retained.admissions.as_ref().unwrap()).unwrap(),
        original
    );
    assert_eq!(serde_json::to_vec(&after.prefix).unwrap(), prefix);
    assert_eq!(serde_json::to_vec(&after.layouts).unwrap(), layouts);
    assert_eq!(after.network.as_ref().map(|c| *c.bytes()), network);
    println!("actual_private_receipt_log_cpu{}: rows=28 hot=6 historical=22 new_allocations={} newly_reserved={} freed={} net_released={} preparation_overlap={} capacities={:?} manager_wire_identical=true checkpoint_reopened=true target_savings_unmeasured=true header_delta={}",usize::BITS,memory.allocations,memory.requested_bytes,memory.deallocated_bytes,saving,memory.peak_bytes,usage.allocation_capacities, core::mem::size_of::<conduit_body::AdmissionReceiptLog>()-core::mem::size_of::<Vec<conduit_body::AdmissionReceipt>>());
    println!("actual_private_receipt_clone_cpu{}: owned_retained={} compact_retained={} owned_peak={} compact_peak={} existing_clone_calls_unchanged=true",usize::BITS,owned_clone_memory.retained_bytes(),compact_clone_memory.retained_bytes(),owned_clone_memory.peak_bytes,compact_clone_memory.peak_bytes);
}
