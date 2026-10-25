//! Inert transcript provider retained through Ready -> Offer -> Plan -> Play.
use conduit_core::*;
use conduit_kernel::NodeId;
use conduit_plan_lowering::{
    lowering::LoweredPlanFragment, selected_operation::SelectedOperationPlan,
};
use conduit_pulse_capture::{installation::*, owner::*, *};

#[path = "pulse/provider.rs"]
mod provider;
pub use provider::*;

pub fn bind(
    ready: ReadyPulseBase<Provider>,
    plan: &Plan,
    lowered: &LoweredPlanFragment,
) -> Result<(PulseHostCall<Provider>, NodeId), PulseCallRefusal> {
    bind_mode(ready, plan, lowered, None, 1)
}
pub fn bind_assigned(
    ready: ReadyPulseBase<Provider>,
    plan: &Plan,
    lowered: &LoweredPlanFragment,
    projection: &super::assignment::NativeProjection,
) -> Result<(PulseHostCall<Provider>, NodeId), PulseCallRefusal> {
    bind_mode(ready, plan, lowered, Some(projection), 1)
}
/// Explicit finite CPU Root budget, independent of received assignment bytes.
pub fn bind_budget(
    ready: ReadyPulseBase<Provider>,
    plan: &Plan,
    lowered: &LoweredPlanFragment,
    operations: u32,
) -> Result<(PulseHostCall<Provider>, NodeId), PulseCallRefusal> {
    bind_mode(ready, plan, lowered, None, operations)
}
fn bind_mode(
    ready: ReadyPulseBase<Provider>,
    plan: &Plan,
    lowered: &LoweredPlanFragment,
    projection: Option<&super::assignment::NativeProjection>,
    operations: u32,
) -> Result<(PulseHostCall<Provider>, NodeId), PulseCallRefusal> {
    let fragment = &plan.fragments[0];
    let gear = fragment
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == PULSE_READ_KIND)
        .unwrap();
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let identity = identity();
    let scope = BaseCapabilityScope {
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: 1,
        plan_id: plan.plan_id.clone(),
        active_play_id: active.active_play_id.clone(),
        authority_grant_id: grant().grant_id,
        authority_contract_id: PULSE_AUTHORITY.into(),
        capability_id: PULSE_CAPABILITY.into(),
        implementation_id: identity.implementation_id.clone(),
        operation_contract_id: PULSE_READ_CALL.into(),
        subject_kind: PULSE_READ_KIND.into(),
        resource_pool_id: identity.resource_pool_id.clone(),
        resource_generation_id: identity.resource_generation_id.clone(),
        envelope_id: identity.envelope_id.clone(),
        maximum_parameter_bytes: 1024,
        maximum_result_bytes: 1024,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: operations,
    };
    let authority = BaseCapabilityAuthority {
        grant: grant(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: 1024,
        maximum_result_bytes: 1024,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: operations,
    };
    let claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: 1024,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        identity.host_id,
        identity.boot_id,
        identity.provider_instance_id,
        1,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    let node = lowered
        .nodes
        .iter()
        .find(|n| n.placement_id == gear.placement_id)
        .unwrap()
        .node;
    let owner = if let Some(projection) = projection {
        ready.bind_assigned(
            table,
            handle,
            claim,
            &PulseReadContract::prepare().unwrap(),
            conduit_plan_lowering::selected_assigned_operation::SelectedAssignedOperationPlan {
                input: conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput::Contiguous(&projection.bytes),
                maxima: super::assignment::NativeProjection::maxima(),
                requirements: projection.required(),
                computations: projection.profile(),
                fragment_id: &fragment.fragment_id,
                active: &active,
                node,
            },
        )?
    } else {
        ready.bind_selected(
            table,
            handle,
            claim,
            &PulseReadContract::prepare().unwrap(),
            SelectedOperationPlan {
                fragment,
                lowered,
                active: &active,
                placement_id: &gear.placement_id,
            },
        )?
    };
    Ok((owner, node))
}
