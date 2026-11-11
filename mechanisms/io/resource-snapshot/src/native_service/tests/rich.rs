//! Production provider selection with the real capacity-one reservation ledger.
use super::*;
use alloc::vec;
use conduit_core::*;
use conduit_plan_lowering::{
    lowering::{LoweredPlanFragment, lower_plan_fragment},
    selected_operation::SelectedOperationPlan,
};
#[path = "../../../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod selection_fixture;
pub struct Fixture {
    pub fragment: PlanFragment,
    pub lowered: LoweredPlanFragment,
    pub active: ActivePlayIdentity,
    pub placement: PlacementId,
    pub scopes: [BaseCapabilityScope; 2],
    pub owner: ResourceAdmissionOwner,
    pub resource: ResourceBinding,
}
pub fn grant(scope: &BaseCapabilityScope) -> AuthorityGrant {
    AuthorityGrant {
        grant_id: scope.authority_grant_id.clone(),
        contract_id: scope.authority_contract_id.clone(),
        host_call_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        capability_id: scope.capability_id.clone(),
    }
}
pub fn operation(scope: BaseCapabilityScope) -> BoundBaseOperation {
    operation_with_claim(scope, |_: &mut BaseOperationClaim| {}).unwrap()
}
pub fn operation_with_claim(
    scope: BaseCapabilityScope,
    change: impl FnOnce(&mut BaseOperationClaim),
) -> Result<BoundBaseOperation, BaseCapabilityRefusal> {
    let authority = BaseCapabilityAuthority {
        grant: grant(&scope),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: scope.maximum_parameter_bytes,
        maximum_result_bytes: scope.maximum_result_bytes,
        maximum_work_units: scope.maximum_work_units,
        maximum_in_flight: scope.maximum_in_flight,
        maximum_operations: scope.maximum_operations,
    };
    let mut claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: scope.maximum_parameter_bytes,
        work_units: scope.maximum_work_units,
    };
    change(&mut claim);
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [19; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    BoundBaseOperation::bind(table, handle, claim).map_err(|(_, _, _, error)| error)
}
impl Fixture {
    pub fn new() -> Self {
        let kind = crate::service_contract::definition();
        let append = BaseCapabilityScope {
            host_id: "host".into(),
            boot_id: "boot".into(),
            base_instance_id: "provider".into(),
            base_provider_generation: 1,
            plan_id: "plan".into(),
            active_play_id: "play".into(),
            authority_grant_id: "grant/append".into(),
            authority_contract_id: crate::native_append::APPEND_AUTHORITY.into(),
            capability_id: SERVICE_CAPABILITY.into(),
            implementation_id: "service@1".into(),
            operation_contract_id: crate::native_append::APPEND_OPERATION.into(),
            subject_kind: crate::append_contract::APPEND_KIND.into(),
            resource_pool_id: "residence".into(),
            resource_generation_id: ResourceGenerationId("generation/1".into()),
            envelope_id: "residence/envelope".into(),
            maximum_parameter_bytes: 200,
            maximum_result_bytes: crate::append_result::APPEND_RESULT_BYTES,
            maximum_work_units: 6,
            maximum_in_flight: 1,
            maximum_operations: 16,
        };
        let mut read = append.clone();
        read.authority_grant_id = "grant/read".into();
        read.authority_contract_id = READ_AUTHORITY.into();
        read.operation_contract_id = READ_OPERATION.into();
        read.subject_kind = crate::read_contract::READ_KIND.into();
        read.maximum_parameter_bytes = 8;
        read.maximum_result_bytes = crate::read_result::READ_RESULT_BYTES as u32;
        read.maximum_work_units = 2;
        let mut scopes = [append, read];
        let (mut fragment, _, _, placement) = selection_fixture::selected(
            &scopes[0],
            &kind,
            crate::native_append::APPEND_OPERATION,
            8,
            "residence",
            "service",
        );
        let gear = &mut fragment.placements[0];
        gear.execution_profile_id = SERVICE_PROFILE.into();
        gear.resources[0].class_id = crate::native_append::APPEND_CLASS.into();
        gear.base.as_mut().unwrap().mechanism_family =
            crate::native_append::APPEND_MECHANISM.into();
        let capability =
            crate::native_service::capability_offer(&gear.artifact_id, &gear.implementation_id)
                .unwrap();
        gear.host_calls = capability.host_calls.clone();
        gear.authority = scopes
            .iter()
            .map(|s| AuthorityBinding {
                grant_id: s.authority_grant_id.clone(),
                contract_id: s.authority_contract_id.clone(),
                host_call_contract_id: s.operation_contract_id.clone(),
                subject_kind: s.subject_kind.clone(),
                host_id: s.host_id.clone(),
                boot_id: s.boot_id.clone(),
                capability_id: s.capability_id.clone(),
            })
            .collect();
        let resource = gear.resources[0].clone();
        let requirement = capability.resource_requirements[0].clone();
        let fragment = seal_plan(
            PlotIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_plot_id: fragment.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
            },
            vec![fragment],
        )
        .fragments
        .remove(0);
        let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        for s in &mut scopes {
            s.plan_id = fragment.plan_id.clone();
            s.active_play_id = active.active_play_id.clone();
        }
        let lowered = lower_plan_fragment(&fragment).unwrap();
        let gear = &fragment.placements[0];
        let host = HostAdvertisement {
            declared_name: None,
            protocol_version: PROTOCOL_VERSION,
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
            profile: "fixture".into(),
            bases: vec![],
            capabilities: vec![capability],
            planner_capabilities: vec![],
            resources: vec![resource_offer(
                resource.pool_id.as_str(),
                resource.class_id.as_str(),
                1,
            )],
        };
        let observations = [ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: resource.pool_id.clone(),
            class_id: resource.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: 1,
            utilized_units: 0,
            sign_id: "current".into(),
        }];
        let mut owner = ResourceAdmissionOwner::new(host);
        owner
            .admit_planned_placement(fragment.plan_id.clone(), gear, &observations)
            .unwrap();
        Self {
            fragment,
            lowered,
            active,
            placement,
            scopes,
            owner,
            resource,
        }
    }
}

fn ready(f: &Fixture) -> ReadyRecordService<Storage> {
    let gear = &f.fragment.placements[0];
    let base = gear.base.as_ref().unwrap();
    let scope = &f.scopes[0];
    let identity = AppendNativeIdentity {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_id: base.base_id.clone(),
        provider_instance_id: base.provider_instance_id.clone(),
        provider_generation: base.provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        artifact_id: gear.artifact_id.clone(),
        implementation_id: gear.implementation_id.clone(),
        base_implementation_id: base.implementation_id.clone(),
    };
    // SAFETY: actual sole CPU residence, exact fresh test grants, bounded effects.
    unsafe {
        ReadyRecordService::new_retained(
            identity,
            super::fixture::recovered_provider(),
            [grant(&f.scopes[0]), grant(&f.scopes[1])],
            ServiceLimits {
                append_operations: 16,
                read_operations: 16,
            },
        )
    }
    .unwrap_or_else(|_| panic!("ready service"))
}
#[test]
fn production_selection_admits_one_residence_and_refuses_foreign_grant_without_losing_owner() {
    let f = Fixture::new();
    let mut slot = Some(ready(&f));
    let mut foreign = f.scopes[1].clone();
    foreign.authority_grant_id = "grant/foreign".into();
    let operations = [operation(f.scopes[0].clone()), operation(foreign)];
    let select = || SelectedOperationPlan {
        fragment: &f.fragment,
        lowered: &f.lowered,
        active: &f.active,
        placement_id: &f.placement,
    };
    let (returned, error) = match ReadyRecordService::prepare_selected_retained(
        &mut slot,
        operations,
        &f.resource,
        select(),
        &f.owner,
    ) {
        Err(value) => value,
        Ok(_) => panic!("foreign grant admitted"),
    };
    assert_eq!(error, ServiceRefusal::Possession);
    assert!(slot.is_some());
    assert_eq!(returned[1].inspection().completed_operations, 0);
    let mut duplicate = f.resource.clone();
    duplicate.units = 2;
    let operations = [
        operation(f.scopes[0].clone()),
        operation(f.scopes[1].clone()),
    ];
    let (returned, _) = match ReadyRecordService::prepare_selected_retained(
        &mut slot,
        operations,
        &duplicate,
        select(),
        &f.owner,
    ) {
        Err(value) => value,
        Ok(_) => panic!("duplicate resource admitted"),
    };
    assert!(slot.is_some());
    assert_eq!(returned[0].inspection().completed_operations, 0);
    let operations = [
        operation(f.scopes[0].clone()),
        operation(f.scopes[1].clone()),
    ];
    let prepared = ReadyRecordService::prepare_selected_retained(
        &mut slot,
        operations,
        &f.resource,
        select(),
        &f.owner,
    )
    .unwrap_or_else(|_| panic!("actual ordinary service selection"));
    let owner = prepared.bind();
    assert!(slot.is_none());
    assert_eq!(owner.node(), NodeId(0));
    assert_eq!(f.owner.admissions().len(), 1);
    assert_eq!(f.owner.admissions()[0].items.len(), 1);
    assert_eq!(f.owner.admissions()[0].items[0].binding.units, 1);
    assert_eq!(f.owner.host().resources[0].capacity_units, 1);
}
