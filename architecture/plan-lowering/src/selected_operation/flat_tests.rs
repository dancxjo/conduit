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
