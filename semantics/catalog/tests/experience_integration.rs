#[allow(dead_code)] // Shared fixture functions serve several independent proof surfaces.
#[path = "common/experience_evidence.rs"]
mod common;
use common::*;
use conduit_human::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_semantic_catalog::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct CountingAllocator;
thread_local! { static COUNTING: Cell<bool> = const { Cell::new(false) }; static ALLOCATIONS: Cell<usize> = const { Cell::new(0) }; }
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNTING.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNTING.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
fn update(revision: u64, operation: ExperienceIntegrationOperation) -> Vec<u8> {
    encode(
        ExperienceIntegrationUpdate::new(revision, operation, identity("snapshot/lineage"))
            .unwrap(),
    )
}
fn upsert(revision: u64, value: ExperienceEvidenceItem) -> Vec<u8> {
    update(
        revision,
        ExperienceIntegrationOperation::upsert(
            *value.availability(),
            *value.certainty(),
            value.content_kind().clone(),
            *value.domain(),
            value.encoded_content().clone(),
            value.identity().clone(),
            *value.origin(),
            value.provenance_revision().clone(),
            *value.revision(),
            value.sources().clone(),
            *value.temporal_role(),
        )
        .unwrap(),
    )
}
fn decode(bytes: &[u8]) -> ExperienceEvidenceRequest {
    ExperienceEvidenceRequest::decode(bytes).unwrap()
}

#[test]
fn snapshots_preserve_evidence_rebind_lineage_and_commit_transactionally() {
    let mut integration = BoundedExperienceIntegration::prepare();
    let observed = observation("observation");
    let input = upsert(0, observed.clone());
    let first = decode(integration.execute(&input).unwrap());
    assert_eq!(*first.selection().revision(), 1);
    assert_eq!(*first.evidence().get().as_slice()[0].revision(), 1);
    assert_eq!(
        first.evidence().get().as_slice()[0].sources(),
        observed.sources()
    );
    assert_eq!(integration.revision(), 0);
    assert_eq!(
        integration.execute(&input),
        Err(ExperienceIntegrationRefusal::PendingUpdate)
    );
    integration.cancel();
    assert_eq!(integration.revision(), 0);
    integration.execute(&input).unwrap();
    integration.commit();
    let remembered = item(
        "memory",
        ExperienceOrigin::Remembered,
        ExperienceTemporalRole::Historical,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        b"remembered content",
    );
    let second = decode(integration.execute(&upsert(1, remembered.clone())).unwrap());
    assert_eq!(second.evidence().get().as_slice().len(), 2);
    for evidence in second.evidence().get().as_slice() {
        assert_eq!(*evidence.revision(), 2);
        assert_eq!(
            evidence.provenance_revision(),
            second.selection().provenance_revision()
        );
    }
    assert_eq!(
        second.evidence().get().as_slice()[1].origin(),
        remembered.origin()
    );
    assert_eq!(
        second.evidence().get().as_slice()[1].encoded_content(),
        remembered.encoded_content()
    );
    integration.commit();
    let replacement = upsert(2, observation("observation"));
    assert_eq!(
        decode(integration.execute(&replacement).unwrap())
            .evidence()
            .get()
            .as_slice()
            .len(),
        2
    );
    integration.commit();
    let removed = update(
        3,
        ExperienceIntegrationOperation::remove(identity("memory")).unwrap(),
    );
    assert_eq!(
        decode(integration.execute(&removed).unwrap())
            .evidence()
            .get()
            .as_slice()
            .len(),
        1
    );
    integration.commit();
    assert_eq!(integration.revision(), 4);
}

#[test]
fn rejected_updates_leave_committed_state_and_revision_intact() {
    let mut integration = BoundedExperienceIntegration::prepare();
    assert_eq!(
        integration.execute(&upsert(1, observation("a"))),
        Err(ExperienceIntegrationRefusal::FutureRevision)
    );
    for revision in 0..8 {
        integration
            .execute(&upsert(revision, observation(&format!("item/{revision}"))))
            .unwrap();
        integration.commit();
    }
    assert_eq!(
        integration.execute(&upsert(8, observation("overflow"))),
        Err(ExperienceIntegrationRefusal::ItemBound)
    );
    assert_eq!(
        integration.execute(&upsert(0, observation("a"))),
        Err(ExperienceIntegrationRefusal::StaleRevision)
    );
    let absent = update(
        8,
        ExperienceIntegrationOperation::remove(identity("absent")).unwrap(),
    );
    assert_eq!(
        integration.execute(&absent),
        Err(ExperienceIntegrationRefusal::UnknownIdentity)
    );
    let invalid = item(
        "item/0",
        ExperienceOrigin::Remembered,
        ExperienceTemporalRole::Current,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        b"false current memory",
    );
    assert_eq!(
        integration.execute(&upsert(8, invalid)),
        Err(ExperienceIntegrationRefusal::Evidence(
            ExperienceEvidenceRefusal::InvalidEpistemicCombination
        ))
    );
    assert_eq!(integration.revision(), 8);
    assert!(integration.output().is_empty());
    let valid = upsert(8, observation("item/0"));
    assert_eq!(
        *decode(integration.execute(&valid).unwrap())
            .selection()
            .revision(),
        9
    );
}

#[test]
fn successive_execution_commit_cancel_and_refusal_allocate_nothing() {
    let mut integration = BoundedExperienceIntegration::prepare();
    let first = upsert(0, observation("a"));
    let next = upsert(1, observation("b"));
    let remove = update(
        2,
        ExperienceIntegrationOperation::remove(identity("a")).unwrap(),
    );
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|flag| flag.set(true));
    let succeeded = integration.execute(&first).is_ok();
    integration.commit();
    let staged = integration.execute(&next).is_ok();
    integration.cancel();
    let restaged = integration.execute(&next).is_ok();
    integration.commit();
    let removed = integration.execute(&remove).is_ok();
    integration.commit();
    let stale = integration.execute(&first) == Err(ExperienceIntegrationRefusal::StaleRevision);
    COUNTING.with(|flag| flag.set(false));
    assert!(succeeded && staged && restaged && removed && stale);
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(integration.revision(), 3);
}

#[test]
fn ordinary_contract_has_closing_flows_and_no_authority() {
    let contract = experience_integration_contract();
    assert_eq!(contract.kind_id.as_str(), "experience/integrate-current");
    assert!(contract
        .inputs
        .iter()
        .chain(contract.outputs.iter())
        .all(|p| p.temporal == conduit_core::PortTemporal::Flow { closes: true }));
    assert_eq!(contract.inputs.len(), 1);
    assert_eq!(contract.outputs.len(), 1);
}
