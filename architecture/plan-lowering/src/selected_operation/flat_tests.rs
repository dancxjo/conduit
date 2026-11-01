use super::*;
use conduit_core::*;
extern crate std;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
std::thread_local! { static LEFT:Cell<Option<usize>>=const{Cell::new(None)}; static COUNT:Cell<usize>=const{Cell::new(0)}; }
struct Alloc;
#[global_allocator]
static ALLOC: Alloc = Alloc;
unsafe impl GlobalAlloc for Alloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        COUNT.with(|n| n.set(n.get() + 1));
        if LEFT.with(|n| match n.get() {
            Some(0) => true,
            Some(v) => {
                n.set(Some(v - 1));
                false
            }
            None => false,
        }) {
            core::ptr::null_mut()
        } else {
            unsafe { System.alloc(l) }
        }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/files/instance"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/one"),
        active_play_id: ActivePlayId::from("play/one"),
        authority_grant_id: AuthorityGrantId::from("grant/visible"),
        authority_contract_id: AuthorityContractId::from("authority/file-read@1"),
        capability_id: CapabilityId::from("file/copy"),
        implementation_id: ImplementationId::from("implementation/file-copy@1"),
        operation_contract_id: HostCallContractId::from("conduit.host/file-read@1"),
        subject_kind: KindId::from("file/source"),
        resource_pool_id: ResourcePoolId::from("files/project/source"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        envelope_id: CapabilityEnvelopeId::from("file/read-only/project-source/max-64"),
        maximum_parameter_bytes: 64,
        maximum_result_bytes: 128,
        maximum_work_units: 8,
        maximum_in_flight: 2,
        maximum_operations: 3,
    }
}

fn authority() -> BaseCapabilityAuthority {
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: AuthorityGrantId::from("grant/visible"),
            contract_id: AuthorityContractId::from("authority/file-read@1"),
            host_call_contract_id: HostCallContractId::from("conduit.host/file-read@1"),
            subject_kind: KindId::from("file/source"),
            host_id: HostId::from("host/one"),
            boot_id: BootId::from("boot/one"),
            capability_id: CapabilityId::from("file/copy"),
        },
        base_instance_id: BaseInstanceId::from("base/files/instance"),
        base_provider_generation: 4,
        resource_pool_id: ResourcePoolId::from("files/project/source"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        operation_contract_id: HostCallContractId::from("conduit.host/file-read@1"),
        envelope_id: CapabilityEnvelopeId::from("file/read-only/project-source/max-64"),
        maximum_parameter_bytes: 256,
        maximum_result_bytes: 512,
        maximum_work_units: 32,
        maximum_in_flight: 4,
        maximum_operations: 10,
    }
}

fn prepared() -> (PreparedFlatDirectOperation, ActivePlayIdentity) {
    let mut scope = scope();
    let active = bind_active_play(&scope.plan_id, &scope.host_id, &scope.boot_id, 1);
    scope.active_play_id = active.active_play_id.clone();
    (
        PreparedFlatDirectOperation {
            expected: scope,
            node: NodeId(7),
        },
        active,
    )
}
fn table() -> BaseCapabilityTable {
    let s = scope();
    BaseCapabilityTable::new(s.host_id, s.boot_id, s.base_instance_id, 4, [7; 32], 1).unwrap()
}
#[test]
fn all_claim_and_backing_allocations_refuse_without_issuance() {
    for n in 0..13 {
        let (p, _) = prepared();
        LEFT.with(|v| v.set(Some(n)));
        let r = p.prepare_issuance(32, 1);
        LEFT.with(|v| v.set(None));
        assert!(matches!(r, Err(SelectedCallRefusal::Possession)));
        assert!(p.prepare_issuance(32, 1).is_ok());
    }
}
#[test]
fn preallocated_issuance_is_allocation_free_and_checks_active_and_storage() {
    let (p, active) = prepared();
    let storage = p.prepare_issuance(32, 1).unwrap();
    let policy = authority();
    let t = table();
    COUNT.with(|n| n.set(0));
    let issued = p
        .issue_prepared(t, (&policy).into(), &active, storage)
        .unwrap();
    assert_eq!(COUNT.with(Cell::get), 0);
    assert_eq!(issued.node, NodeId(7));
    let (p, mut active) = prepared();
    let storage = p.prepare_issuance(32, 1).unwrap();
    active.boot_id = "wrong".into();
    assert!(matches!(
        p.issue_prepared(table(), (&policy).into(), &active, storage),
        Err(SelectedCallRefusal::WrongBinding)
    ));
    let (p, active) = prepared();
    let mut storage = p.prepare_issuance(32, 1).unwrap();
    storage.claim.envelope_id = "wrong".into();
    assert!(matches!(
        p.issue_prepared(table(), (&policy).into(), &active, storage),
        Err(SelectedCallRefusal::WrongBinding)
    ));
}

#[test]
fn reusable_claim_reserves_maxima_refills_and_keeps_refusal_custody() {
    let (first, active) = prepared();
    let (mut second, _) = prepared();
    second.expected.implementation_id = "implementation/a-much-longer-second-operation".into();
    for n in 0..11 {
        LEFT.with(|v| v.set(Some(n)));
        let result = PreparedFlatClaimBacking::try_new(&[&first, &second]);
        LEFT.with(|v| v.set(None));
        assert!(matches!(result, Err(SelectedCallRefusal::Possession)));
    }
    let memory = PreparedFlatClaimBacking::try_new(&[&first, &second]).unwrap();
    let core1 = BaseCapabilityIssuanceBacking::try_new(2).unwrap();
    let core2 = BaseCapabilityIssuanceBacking::try_new(2).unwrap();
    let policy = authority();
    let t1 = table();
    let t2 = table();
    COUNT.with(|n| n.set(0));
    let storage = first
        .prepare_issuance_reusing(memory, core1, 32, 1)
        .unwrap_or_else(|_| panic!("admitted"));
    let issued = first
        .issue_prepared(t1, (&policy).into(), &active, storage)
        .unwrap();
    let (_, _, memory) = issued
        .bind_recycling()
        .unwrap_or_else(|_| panic!("exact issued"));
    let storage = second
        .prepare_issuance_reusing(memory, core2, 999, 1)
        .unwrap_or_else(|_| panic!("admitted"));
    let issued = second
        .issue_prepared(t2, (&policy).into(), &active, storage)
        .unwrap();
    let (issued, reason) = issued.bind_recycling().err().unwrap();
    assert_eq!(reason, SelectedCallRefusal::Possession);
    assert_eq!(issued.claim.parameter_bytes, 999);
    assert_eq!(issued.table.inspections().count(), 1);
    assert_eq!(COUNT.with(Cell::get), 0);
}
#[test]
fn insufficient_reusable_claim_capacity_returns_memory_unchanged() {
    let (short, _) = prepared();
    let (mut long, _) = prepared();
    long.expected.host_id = "a-longer-host-identity-than-prepared".into();
    let memory = PreparedFlatClaimBacking::try_new(&[&short]).unwrap();
    let capacity = memory.claim.host_id.0.capacity();
    let backing = BaseCapabilityIssuanceBacking::try_new(2).unwrap();
    COUNT.with(|n| n.set(0));
    let (memory, _, reason) = long
        .prepare_issuance_reusing(memory, backing, 32, 1)
        .err()
        .unwrap();
    assert_eq!(reason, SelectedCallRefusal::Possession);
    assert_eq!(memory.claim.host_id.0.capacity(), capacity);
    assert!(memory.claim.host_id.as_str().is_empty());
    assert_eq!(COUNT.with(Cell::get), 0);
}
