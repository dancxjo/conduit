//! CPU native append provider and exact assigned-image fixture.
use conduit_core::*;
use conduit_kernel::NodeId;
use conduit_resource_snapshot::{native_append::*, nor::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[path = "../../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
pub(super) mod selection_fixture;
#[derive(Clone)]
pub(super) struct Storage {
    bytes: Rc<RefCell<Vec<u8>>>,
    pub(super) programs: Rc<Cell<usize>>,
    pub(super) fail: Rc<Cell<bool>>,
    pub(super) fail_commit_read: Rc<Cell<bool>>,
}
impl Storage {
    pub(super) fn new() -> Self {
        Self {
            bytes: Rc::new(RefCell::new(vec![255; 4096])),
            programs: Rc::new(Cell::new(0)),
            fail: Rc::new(Cell::new(false)),
            fail_commit_read: Rc::new(Cell::new(false)),
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
        if self.fail_commit_read.get() && self.programs.get() >= 2 {
            return Err(NorRefusal::Media);
        }
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
pub(super) fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: "snapshot-host".into(),
        boot_id: "boot/native".into(),
        base_instance_id: "snapshot/instance".into(),
        base_provider_generation: 1,
        plan_id: "plan".into(),
        active_play_id: "play".into(),
        authority_grant_id: "grant/snapshot".into(),
        authority_contract_id: APPEND_AUTHORITY.into(),
        capability_id: APPEND_CAPABILITY.into(),
        implementation_id: "snapshot/native@1".into(),
        operation_contract_id: APPEND_OPERATION.into(),
        subject_kind: APPEND_KIND.into(),
        resource_pool_id: "snapshot/pool".into(),
        resource_generation_id: ResourceGenerationId("generation/1".into()),
        envelope_id: "snapshot/window".into(),
        maximum_parameter_bytes: 200,
        maximum_result_bytes: APPEND_RESULT_BYTES,
        maximum_work_units: 128,
        maximum_in_flight: 1,
        maximum_operations: 16,
    }
}
pub(super) fn grant(s: &BaseCapabilityScope) -> AuthorityGrant {
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
        maximum_operations: s.maximum_operations,
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
        parameter_bytes: 200,
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
pub(super) struct Fixture {
    kind: Kind,
    plan: Plan,
    pub(super) storage: Storage,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let kind = conduit_semantic_catalog::resource_append_semantic_contract();
        kind.validate().unwrap();
        let s = scope();
        let (mut fragment, _, _, _) =
            selection_fixture::selected(&s, &kind, APPEND_OPERATION, 200, "snapshot", "snapshot");
        let p = &mut fragment.placements[0];
        p.execution_profile_id = APPEND_PROFILE.into();
        p.host_calls[0].maximum_input_bytes = 200;
        p.host_calls[0].maximum_output_bytes = APPEND_RESULT_BYTES;
        p.resources[0].class_id = APPEND_CLASS.into();
        p.base.as_mut().unwrap().mechanism_family = APPEND_MECHANISM.into();
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
            storage: Storage::new(),
        }
    }
    pub(super) fn ready(&self) -> ReadyRecordAppender<Storage> {
        self.try_ready().unwrap()
    }
    fn try_ready(&self) -> Result<ReadyRecordAppender<Storage>, AppendCallRefusal> {
        let p = &self.plan.fragments[0].placements[0];
        let s = scope();
        let id = AppendNativeIdentity {
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
        let mut provider = conduit_resource_snapshot::append::PreparedNorRecordResidence::prepare(
            self.storage.clone(),
            conduit_resource_snapshot::append::RecordResidenceId([3; 32]),
            0,
            8,
        )
        .unwrap_or_else(|_| panic!("geometry"));
        assert_eq!(provider.recover().unwrap().committed, 0);
        // SAFETY: CPU-only exclusive NOR fixture, bounded synchronous effects.
        unsafe { ReadyRecordAppender::new(id, provider, &grant(&scope()), 16) }
    }
    pub(super) fn bind(
        &self,
        foreign: bool,
    ) -> Result<RecordAppendHostCall<Storage>, AppendCallRefusal> {
        self.bind_mode(foreign, false)
    }
    pub(super) fn bind_mode(
        &self,
        foreign: bool,
        fore: bool,
    ) -> Result<RecordAppendHostCall<Storage>, AppendCallRefusal> {
        self.bind_with_limit(foreign, fore, 16)
    }
    pub(super) fn bind_with_limit(
        &self,
        foreign: bool,
        fore: bool,
        maximum_operations: u32,
    ) -> Result<RecordAppendHostCall<Storage>, AppendCallRefusal> {
        self.with_prepared(foreign, fore, maximum_operations, |ticket| ticket.bind())
    }
    pub(super) fn with_prepared<U>(
        &self,
        foreign: bool,
        fore: bool,
        maximum_operations: u32,
        bind: impl for<'a> FnOnce(PreparedAssignedAppender<'a, Storage>) -> U,
    ) -> Result<U, AppendCallRefusal> {
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
        let calls = [AssignedIdentity::from_text(APPEND_OPERATION)];
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
        let mut scope = scope();
        scope.maximum_operations = maximum_operations;
        let (table, handle, claim) = authorization(scope, fragment, &active);
        let mut slot = Some(self.ready());
        let ticket = if fore {
            ReadyRecordAppender::prepare_assigned_fore_retained(
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
            ReadyRecordAppender::prepare_assigned_retained(
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
            Ok(ticket) => Ok(bind(ticket)),
            Err(e) => {
                assert!(slot.is_some());
                Err(e)
            }
        }
    }
}
