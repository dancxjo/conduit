//! Actual native binder/rotating capability and NOR effects with CPU providers.
extern crate alloc;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_resource_snapshot::{native::*, nor::*, *};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/fixture.rs"]
mod record_fixture;
#[path = "../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod selection_fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[derive(Clone)]
struct Storage {
    bytes: Rc<RefCell<Vec<u8>>>,
    programs: Rc<Cell<usize>>,
    fail: Rc<Cell<bool>>,
}
impl Storage {
    fn new() -> Self {
        Self {
            bytes: Rc::new(RefCell::new(vec![255; 4096])),
            programs: Rc::new(Cell::new(0)),
            fail: Rc::new(Cell::new(false)),
        }
    }
}
impl NorStorage for Storage {
    fn capacity(&self) -> u32 {
        4096
    }
    fn program_unit(&self) -> u32 {
        256
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), NorRefusal> {
        out.copy_from_slice(&self.bytes.borrow()[at as usize..at as usize + out.len()]);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), NorRefusal> {
        self.programs.set(self.programs.get() + 1);
        if self.fail.get() {
            return Err(NorRefusal::Media);
        }
        for (old, new) in self.bytes.borrow_mut()[at as usize..].iter_mut().zip(bytes) {
            assert_eq!(*old & *new, *new);
            *old &= *new;
        }
        Ok(())
    }
}
fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: "snapshot-host".into(),
        boot_id: "boot/native".into(),
        base_instance_id: "snapshot/instance".into(),
        base_provider_generation: 1,
        plan_id: "plan".into(),
        active_play_id: "play".into(),
        authority_grant_id: "grant/snapshot".into(),
        authority_contract_id: AUTHORITY_CONTRACT.into(),
        capability_id: SNAPSHOT_CAPABILITY.into(),
        implementation_id: "snapshot/native@1".into(),
        operation_contract_id: PUBLISH_OPERATION.into(),
        subject_kind: SNAPSHOT_KIND.into(),
        resource_pool_id: "snapshot/pool".into(),
        resource_generation_id: ResourceGenerationId("generation/1".into()),
        envelope_id: "snapshot/window".into(),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 512,
        maximum_work_units: 128,
        maximum_in_flight: 1,
        maximum_operations: 1,
    }
}
fn grant(s: &BaseCapabilityScope) -> AuthorityGrant {
    AuthorityGrant {
        grant_id: s.authority_grant_id.clone(),
        contract_id: s.authority_contract_id.clone(),
        host_call_contract_id: s.operation_contract_id.clone(),
        subject_kind: s.subject_kind.clone(),
        host_id: s.host_id.clone(),
        boot_id: s.boot_id.clone(),
        capability_id: s.capability_id.clone(),
    }
}
fn authorization(
    mut s: BaseCapabilityScope,
    fragment: &PlanFragment,
    active: &ActivePlayIdentity,
) -> (
    BaseCapabilityTable,
    BaseCapabilityHandle,
    BaseOperationClaim,
) {
    s.plan_id = fragment.plan_id.clone();
    s.active_play_id = active.active_play_id.clone();
    let a = BaseCapabilityAuthority {
        grant: grant(&s),
        base_instance_id: s.base_instance_id.clone(),
        base_provider_generation: 1,
        resource_pool_id: s.resource_pool_id.clone(),
        resource_generation_id: s.resource_generation_id.clone(),
        operation_contract_id: s.operation_contract_id.clone(),
        envelope_id: s.envelope_id.clone(),
        maximum_parameter_bytes: s.maximum_parameter_bytes,
        maximum_result_bytes: s.maximum_result_bytes,
        maximum_work_units: s.maximum_work_units,
        maximum_in_flight: 1,
        maximum_operations: 1,
    };
    let claim = BaseOperationClaim {
        host_id: s.host_id.clone(),
        boot_id: s.boot_id.clone(),
        base_instance_id: s.base_instance_id.clone(),
        base_provider_generation: 1,
        plan_id: s.plan_id.clone(),
        active_play_id: s.active_play_id.clone(),
        implementation_id: s.implementation_id.clone(),
        operation_contract_id: s.operation_contract_id.clone(),
        subject_kind: s.subject_kind.clone(),
        resource_pool_id: s.resource_pool_id.clone(),
        resource_generation_id: s.resource_generation_id.clone(),
        envelope_id: s.envelope_id.clone(),
        parameter_bytes: 4096,
        work_units: 64,
    };
    let mut table = BaseCapabilityTable::new(
        s.host_id.clone(),
        s.boot_id.clone(),
        s.base_instance_id.clone(),
        1,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest {
            scope: s,
            authority: a,
        })
        .unwrap();
    (table, handle, claim)
}
struct Fixture {
    kind: Kind,
    plan: Plan,
    reference: BoundedResourceRef,
    storage: Storage,
}
impl Fixture {
    fn new() -> Self {
        let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(true);
        let s = scope();
        let (mut fragment, _, _, _) =
            selection_fixture::selected(&s, &kind, PUBLISH_OPERATION, 4096, "snapshot", "snapshot");
        let (template, reference) = record_fixture::placement("boot/native", true, 2);
        let p = &mut fragment.placements[0];
        p.execution_profile_id = SNAPSHOT_PROFILE.into();
        p.host_calls[0].maximum_output_bytes = 512;
        p.resources[0] = template.resources[0].clone();
        p.resources[0].pool_id = s.resource_pool_id;
        p.resources[0].content.as_mut().unwrap().base_id = "base/snapshot".into();
        p.base.as_mut().unwrap().mechanism_family = SNAPSHOT_MECHANISM.into();
        let encoded = reference.encode().unwrap();
        let text = encoded
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        p.configuration = vec![ConfigurationEntry {
            key: "reference".into(),
            value: ConfigurationValue::Text(text),
        }];
        let plan = seal_plan(
            PlotIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_plot_id: fragment.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
            },
            vec![fragment],
        );
        Self {
            kind,
            plan,
            reference,
            storage: Storage::new(),
        }
    }
    fn ready(&self) -> ReadySnapshotPublisher<Storage> {
        self.try_ready().unwrap()
    }
    fn try_ready(&self) -> Result<ReadySnapshotPublisher<Storage>, SnapshotCallRefusal> {
        let p = &self.plan.fragments[0].placements[0];
        let s = scope();
        let local = ResourceReferenceBinding {
            identity: self.reference.identity,
            version: self.reference.lifetime.version,
            content_profile: self.reference.content_profile.clone(),
            access_class: self.reference.access_class.clone(),
            handle: "physical/slot".into(),
            authority_contract: AUTHORITY_CONTRACT.into(),
            authority_grant: s.authority_grant_id.clone(),
            maximum_bytes: 4096,
            maximum_items: Some(1),
            availability: ResourceReferenceAvailability::Available,
        };
        let id = SnapshotNativeIdentity {
            host_id: s.host_id,
            boot_id: s.boot_id,
            base_id: "base/snapshot".into(),
            provider_instance_id: s.base_instance_id,
            provider_generation: 1,
            resource_pool_id: s.resource_pool_id,
            resource_generation_id: s.resource_generation_id,
            envelope_id: s.envelope_id,
            artifact_id: p.artifact_id.clone(),
            implementation_id: p.implementation_id.clone(),
            base_implementation_id: p.base.as_ref().unwrap().implementation_id.clone(),
        };
        // SAFETY: exclusive in-memory NOR fixture, synchronous bounded effects.
        unsafe {
            ReadySnapshotPublisher::new(
                id,
                PreparedNorSnapshotWriter::prepare(self.storage.clone())
                    .unwrap_or_else(|_| panic!("geometry")),
                SnapshotRecordSelection {
                    host: &p.host_id,
                    boot: &p.boot_id,
                    kind: &p.kind_id,
                    capability: &p.capability_id,
                    binding: &p.resources[0],
                    host_call: &p.host_calls[0],
                    authority: &p.authority[0],
                },
                &self.reference,
                &grant(&scope()),
                &local,
            )
        }
    }
    fn bind(&self, foreign: bool) -> Result<SnapshotPublishHostCall<Storage>, SnapshotCallRefusal> {
        self.bind_mode(foreign, false)
    }
    fn bind_mode(
        &self,
        foreign: bool,
        fore: bool,
    ) -> Result<SnapshotPublishHostCall<Storage>, SnapshotCallRefusal> {
        use conduit_embedded_build as e;
        use conduit_plan_lowering::{
            assigned_play_scope::AssignedPlayScope, selected_assigned_operation::*,
        };
        let fragment = &self.plan.fragments[0];
        let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
        let bounds = e::EmbeddedImageBounds {
            maximum_nodes: 1,
            maximum_cords: 0,
            maximum_routes: 2,
            maximum_route_targets: 2,
            maximum_host_calls: 1,
            maximum_resources: 1,
            maximum_sign_expectations: 16,
            maximum_configuration_entries: 1,
            maximum_ports_per_node: 8,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: 2,
            maximum_cord_value_bytes: 4096,
            maximum_sign_items: 64,
            maximum_sign_bytes: 8192,
        };
        let image = e::generate_embedded_plan(fragment, &lowered, bounds).unwrap();
        let maxima = AssignedPlanMaxima {
            encoded_bytes: 8192,
            runtime_state_bytes: 16384,
            counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
        };
        let bytes = if fore {
            e::generate_fore_native_plan(
                &self.plan,
                &fragment.fragment_id,
                bounds,
                e::EmbeddedComputationBounds {
                    maximum_program_bytes: 32768,
                    maximum_total_program_bytes: 32768,
                },
                0,
            )
            .unwrap()
            .encode(maxima)
            .unwrap()
        } else {
            e::encode_assigned_native_plan(&image, maxima).unwrap()
        };
        // Fabricated trusted image inventory, never derived from received bytes.
        let artifacts = e::assigned_computation_requirements(&image).unwrap();
        let text_bytes = artifacts.iter().map(|pin| pin.text_bytes).sum();
        let calls = [AssignedIdentity::from_text(PUBLISH_OPERATION)];
        let resources: Vec<_> = image.resources.iter().map(|r| r.resource).collect();
        let mut selected_active = active.clone();
        if foreign {
            selected_active.boot_id = "foreign".into();
        }
        let selected = SelectedAssignedOperationPlan {
            input: AssignedOperationInput::Contiguous(&bytes),
            maxima,
            requirements: AssignedPlanRequirements {
                host: AssignedIdentity::from_text(fragment.host_id.as_str()),
                boot: AssignedIdentity::from_text(fragment.boot_id.as_str()),
                host_calls: &calls,
                resources: &resources,
                remote_bindings: &[],
            },
            computations: AssignedComputationProfile {
                maximum_text_bytes: text_bytes,
                maximum_total_text_bytes: text_bytes,
                requirements: &artifacts,
            },
            fragment_id: fragment.fragment_id.as_str(),
            scope: AssignedPlayScope::Plot(&selected_active),
            node: NodeId(0),
        };
        let (table, handle, claim) = authorization(scope(), fragment, &active);
        let mut slot = Some(self.ready());
        let ticket = if fore {
            ReadySnapshotPublisher::prepare_assigned_fore_retained(
                &mut slot,
                table,
                handle,
                claim,
                &self.kind,
                &fragment.placements[0].configuration,
                SelectedAssignedForeOperationPlan {
                    operation: selected,
                    fore: &[],
                },
            )
        } else {
            ReadySnapshotPublisher::prepare_assigned_retained(
                &mut slot,
                table,
                handle,
                claim,
                &self.kind,
                &fragment.placements[0].configuration,
                selected,
            )
        };
        match ticket {
            Ok(ticket) => Ok(ticket.bind()),
            Err(e) => {
                assert!(slot.is_some());
                Err(e)
            }
        }
    }
}
#[test]
fn actual_native_assignment_preserves_slot_on_refusal_then_one_shot_publication_is_recoverable() {
    let f = Fixture::new();
    assert!(f.bind(true).is_err());
    assert_eq!(f.storage.programs.get(), 0);
    let mut owner = f.bind(false).unwrap();
    let mut out = [0; 512];
    assert_eq!(
        owner.invoke(NodeId(0), HostCallId(0), RequestId(0), b"[]", &mut []),
        Err(SnapshotCallRefusal::OutputPressure)
    );
    assert_eq!(f.storage.programs.get(), 0);
    let (result, cost) = allocation_probe::observe(|| {
        owner.invoke(NodeId(0), HostCallId(0), RequestId(0), b"[]", &mut out)
    });
    let n = result.unwrap();
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(&out[..n], f.reference.encode().unwrap());
    let programs = f.storage.programs.get();
    assert!(programs > 0);
    assert_eq!(owner.inspection().completed_operations, 1);
    assert_eq!(owner.inspection().in_flight_operations, 0);
    assert_eq!(
        owner
            .invoke(NodeId(0), HostCallId(0), RequestId(0), b"[]", &mut out)
            .unwrap(),
        n
    );
    assert_eq!(f.storage.programs.get(), programs);
    assert_eq!(
        owner.invoke(NodeId(0), HostCallId(0), RequestId(0), b"{}", &mut out),
        Err(SnapshotCallRefusal::StaleRequest)
    );
    assert_eq!(
        owner.invoke(NodeId(0), HostCallId(0), RequestId(1), b"[]", &mut out),
        Err(SnapshotCallRefusal::StaleRequest)
    );
    owner.revoke().unwrap();
    assert!(owner
        .invoke(NodeId(0), HostCallId(0), RequestId(0), b"[]", &mut out)
        .is_err());
    assert_eq!(f.storage.programs.get(), programs);
    // Losing every kernel result still leaves the exact media record readable.
    let (p, _) = record_fixture::placement("boot/fresh", false, 2);
    let record = PreparedSnapshotRecord::prepare(&p, &f.reference).unwrap();
    let mut retained = Some(owner);
    let ready = SnapshotPublishHostCall::retire_retained(&mut retained).unwrap();
    assert!(retained.is_none());
    let mut reopened = PreparedNorSnapshot::from_writer(ready.into_provider());
    assert_eq!(reopened.read(&record, &p.authority[0]).unwrap(), b"[]");
}
#[test]
fn malformed_input_stale_calls_cancel_and_media_failure_never_retry_writes() {
    let f = Fixture::new();
    let mut owner = f.bind(false).unwrap();
    let mut out = [0; 512];
    for (node, call, input) in [
        (NodeId(1), HostCallId(0), b"[]".as_slice()),
        (NodeId(0), HostCallId(1), b"[]".as_slice()),
        (NodeId(0), HostCallId(0), b"xx".as_slice()),
    ] {
        assert!(owner
            .invoke(node, call, RequestId(0), input, &mut out)
            .is_err());
    }
    assert_eq!(f.storage.programs.get(), 0);
    f.storage.fail.set(true);
    assert!(owner
        .invoke(NodeId(0), HostCallId(0), RequestId(0), b"[]", &mut out)
        .is_err());
    let count = f.storage.programs.get();
    f.storage.fail.set(false);
    assert!(owner
        .invoke(NodeId(0), HostCallId(0), RequestId(0), b"[]", &mut out)
        .is_err());
    assert_eq!(f.storage.programs.get(), count);
    let f = Fixture::new();
    let mut owner = f.bind(false).unwrap();
    owner.revoke().unwrap();
    assert!(owner
        .invoke(NodeId(0), HostCallId(0), RequestId(0), b"[]", &mut out)
        .is_err());
    assert_eq!(f.storage.programs.get(), 0);
}

#[test]
fn selected_record_must_fit_actual_slot_before_native_installation() {
    let mut f = Fixture::new();
    f.reference.extent.bytes = 4096;
    assert!(matches!(
        f.try_ready(),
        Err(SnapshotCallRefusal::Media(NorRefusal::Capacity))
    ));
    assert_eq!(f.storage.programs.get(), 0);
}

#[test]
fn exact_local_fore_profile_binds_without_retaining_kind_or_configuration() {
    let f = Fixture::new();
    assert!(f.bind_mode(true, true).is_err());
    let mut owner = f.bind_mode(false, true).unwrap();
    let mut out = [0; 512];
    let n = owner
        .invoke(NodeId(0), HostCallId(0), RequestId(0), b"[]", &mut out)
        .unwrap();
    assert_eq!(&out[..n], f.reference.encode().unwrap());
    assert_eq!(owner.inspection().completed_operations, 1);
}
