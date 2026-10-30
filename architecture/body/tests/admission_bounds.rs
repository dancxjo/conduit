use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountingAllocator;
thread_local! {
    static OBSERVING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn record_allocation() {
    if OBSERVING.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn inspect_without_allocation(manager: &AdmissionManager) -> conduit_body::AdmissionStorageUsage {
    ALLOCATIONS.with(|count| count.set(0));
    OBSERVING.with(|active| active.set(true));
    let usage = manager.storage_usage();
    OBSERVING.with(|active| active.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    usage
}

use conduit_body::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, Body, BodyMembership,
    CandidateInventory, CandidateObservation, CandidateRefusal, DiscoveryProofId,
    MAX_PENDING_ADMISSIONS,
};
use conduit_core::{
    BootId, CheckedPlotId, HostAdvertisement, HostId, HostProfileId, LinkBindingId,
    OfferGeneration, SignId, SourceDocumentId, PROTOCOL_VERSION,
};
use ed25519_dalek::{Signer, SigningKey};

const NOW: u64 = 10_000;
const EXPIRES: u64 = 20_000;

fn advertisement(index: usize) -> HostAdvertisement {
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("host/pressure/{index}")),
        boot_id: BootId::from(format!("boot/pressure/{index}")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("profile/admission-pressure"),
        bases: vec![],
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    }
}

fn observation(index: usize) -> CandidateObservation {
    CandidateObservation {
        advertisement: advertisement(index),
        friendly_label: "bounded peer".into(),
        observed_binding_id: LinkBindingId::from(format!("line/pressure/{index}")),
        observation_sign_id: SignId::from(format!("sign/pressure/{index}/observed")),
        proof_id: DiscoveryProofId::bind(&format!("proof/pressure/{index}")).unwrap(),
        freshness_sequence: 1,
        encoded_bytes: 512,
    }
}

#[test]
fn ambient_admission_storage_pressure_refuses_before_membership() {
    let body = Body::born(
        SourceDocumentId::from("source/admission-pressure"),
        CheckedPlotId::from("checked/admission-pressure"),
        1,
        SignId::from("sign/admission-pressure/body-born"),
    )
    .unwrap();
    let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let mut membership = BodyMembership::new(body.body_id).unwrap();
    let key = SigningKey::from_bytes(&[17; 32]);

    let empty = inspect_without_allocation(&manager);
    assert_eq!(empty.pending_admissions.len, 0);
    assert_eq!(empty.receipts.len, 0);

    let mut first_challenge = None;
    for index in 0..MAX_PENDING_ADMISSIONS {
        let candidate_id = candidates.observe(observation(index)).unwrap();
        let challenge = manager
            .begin_ambient(
                &mut candidates,
                &candidate_id,
                key.verifying_key().to_bytes(),
                [u8::try_from(index + 1).unwrap(); 32],
                NOW + u64::try_from(index).unwrap(),
                EXPIRES + u64::try_from(index).unwrap(),
                SignId::from(format!("sign/pressure/{index}/requested")),
            )
            .unwrap();
        if index == 0 {
            first_challenge = Some(challenge);
        }
        let usage = inspect_without_allocation(&manager);
        assert_eq!(usage.pending_admissions.len, index + 1);
        assert!(usage.pending_admissions.capacity >= usage.pending_admissions.len);
        assert!(usage.pending_admissions.element_bytes > 0);
        assert_eq!(usage.invitations, empty.invitations);
        assert_eq!(usage.continuity_keys, empty.continuity_keys);
        assert_eq!(usage.pending_returns, empty.pending_returns);
        assert_eq!(usage.receipts, empty.receipts);
    }

    assert_eq!(candidates.candidates.len(), MAX_PENDING_ADMISSIONS);
    assert_eq!(
        candidates.observe(observation(MAX_PENDING_ADMISSIONS)),
        Err(CandidateRefusal::CandidateCapacityExhausted)
    );
    assert!(membership.parts.is_empty());
    let before = inspect_without_allocation(&manager);
    let challenge = first_challenge.unwrap();
    let proof = AmbientAdmissionProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()).to_bytes(),
    };
    manager
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &proof,
            NOW + 1,
            AdmissionSigns {
                part_admitted: SignId::from("sign/pressure/part"),
                host_attached: SignId::from("sign/pressure/host"),
                candidate_admitted: SignId::from("sign/pressure/candidate"),
            },
        )
        .unwrap();
    let completed = inspect_without_allocation(&manager);
    assert_eq!(
        completed.pending_admissions.len,
        before.pending_admissions.len - 1
    );
    assert_eq!(
        completed.pending_admissions.capacity,
        before.pending_admissions.capacity
    );
    assert_eq!(completed.receipts.len, 1);
    assert_eq!(completed.receipts.len, manager.receipts.len());
    assert_eq!(completed.receipts.capacity, manager.receipts.capacity());
    assert_eq!(
        completed.receipts.element_bytes,
        core::mem::size_of::<conduit_body::AdmissionReceipt>()
    );
    assert_eq!(completed.invitations, before.invitations);
    assert_eq!(
        completed.continuity_keys.len,
        before.continuity_keys.len + 1
    );
    assert!(completed.continuity_keys.capacity >= completed.continuity_keys.len);
    assert!(completed.continuity_keys.element_bytes > 0);
    assert_eq!(completed.pending_returns, before.pending_returns);
}
