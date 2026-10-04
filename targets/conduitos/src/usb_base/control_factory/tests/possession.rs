//! Explicit fixture issuance; this does not establish physical ownership.
use super::*;
use crate::{
    machine_membrane::selected_operation::SelectedOperationPlan,
    usb_base::control_owner::{ControlAttachment, ControlCallOwner},
};
use conduit_plan_lowering::lowering::lower_plan_fragment;

pub(super) fn owner(plan: &Plan) -> ControlCallOwner {
    let fragment = &plan.fragments[0];
    let gear = &fragment.placements[0];
    let base = gear.base.as_ref().unwrap();
    let grant = &gear.authority[0];
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let scope = BaseCapabilityScope {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_instance_id: base.provider_instance_id.clone(),
        base_provider_generation: base.provider_generation,
        plan_id: plan.plan_id.clone(),
        active_play_id: active.active_play_id.clone(),
        authority_grant_id: grant.grant_id.clone(),
        authority_contract_id: grant.contract_id.clone(),
        capability_id: gear.capability_id.clone(),
        implementation_id: gear.implementation_id.clone(),
        operation_contract_id: CONTROL_CALL.into(),
        subject_kind: gear.kind_id.clone(),
        resource_pool_id: gear.resources[0].pool_id.clone(),
        resource_generation_id: ResourceGenerationId("fixture/generation".into()),
        envelope_id: "fixture/envelope".into(),
        maximum_parameter_bytes: CONTROL_MAXIMUM_BYTES,
        maximum_result_bytes: CONTROL_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: grant.grant_id.clone(),
            contract_id: grant.contract_id.clone(),
            host_call_contract_id: grant.host_call_contract_id.clone(),
            subject_kind: grant.subject_kind.clone(),
            host_id: grant.host_id.clone(),
            boot_id: grant.boot_id.clone(),
            capability_id: grant.capability_id.clone(),
        },
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: CONTROL_MAXIMUM_BYTES,
        maximum_result_bytes: CONTROL_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    let claim = BaseOperationClaim {
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
        parameter_bytes: CONTROL_MAXIMUM_BYTES,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [71; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    let lowered = lower_plan_fragment(fragment).unwrap();
    // SAFETY: the test is inert; no DMA, controller or actual device is reachable.
    unsafe {
        ControlCallOwner::bind_admitted(
            table,
            handle,
            claim,
            ControlAttachment {
                slot: 1,
                generation: 1,
                maximum_data_bytes: 256,
                resource_bytes: 4096,
            },
            &ControlContract::prepare().unwrap(),
            SelectedOperationPlan {
                fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &gear.placement_id,
            },
        )
    }
    .unwrap()
}
