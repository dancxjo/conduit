//! Shared preparation checks for exact admitted single-call machine Backs.
use conduit_core::{
    ActivePlayIdentity, BaseCapabilityTable, BaseOperationClaim, Kind, PlacementId, PlanFragment,
};
use conduit_kernel::{HostCallId, NodeId, PortId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(crate) struct SelectedOperationContract<'a> {
    pub kind: &'a Kind,
    pub call: &'a str,
    pub input_bytes: u32,
    pub output_bytes: u32,
    pub resource_bytes: u64,
}

pub(crate) struct SelectedOperationPlan<'a> {
    pub fragment: &'a PlanFragment,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a ActivePlayIdentity,
    pub placement_id: &'a PlacementId,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SelectedCallRefusal {
    WrongBinding,
    Possession,
}

pub(crate) fn bind_selected_operation(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    selection: SelectedOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    let SelectedOperationPlan {
        fragment,
        lowered,
        active,
        placement_id,
    } = selection;
    let expected = definition.kind;
    if !conduit_core::verify_plan_fragment(fragment)
        || fragment.plan_id != claim.plan_id
        || fragment.host_id != claim.host_id
        || fragment.boot_id != claim.boot_id
        || active.plan_id != claim.plan_id
        || active.host_id != claim.host_id
        || active.boot_id != claim.boot_id
        || active.active_play_id != claim.active_play_id
        || conduit_core::bind_active_play(
            &claim.plan_id,
            &claim.host_id,
            &claim.boot_id,
            active.play_sequence,
        ) != *active
        || lowered.identity.plan_id != fragment.plan_id
        || lowered.identity.fragment_id != fragment.fragment_id
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    // Matching IDs alone do not prove that numeric routing/storage/terminal
    // tables came from this fragment. Preparation may allocate; Play may
    // only use the exact ordinary lowering, with no substituted tables.
    if conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
        .map_err(|_| SelectedCallRefusal::WrongBinding)?
        != *lowered
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let mut matches = fragment
        .placements
        .iter()
        .filter(|gear| &gear.placement_id == placement_id);
    let gear = matches.next().ok_or(SelectedCallRefusal::WrongBinding)?;
    if matches.next().is_some() {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let base = gear
        .base
        .as_ref()
        .ok_or(SelectedCallRefusal::WrongBinding)?;
    if gear.kind_id != expected.kind_id
        || gear.kind_contract_revision != expected.kind_contract_revision
        || gear.inputs != expected.inputs
        || gear.outputs != expected.outputs
        || !gear.configuration.is_empty()
        || gear.host_id != claim.host_id
        || gear.boot_id != claim.boot_id
        || gear.implementation_id != claim.implementation_id
        || base.provider_instance_id != claim.base_instance_id
        || base.provider_generation != claim.base_provider_generation
        || claim.subject_kind.as_str() != expected.kind_id.as_str()
        || claim.operation_contract_id.as_str() != definition.call
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let inspection = table
        .inspections()
        .next()
        .ok_or(SelectedCallRefusal::Possession)?;
    if inspection.lifecycle != conduit_core::CapabilityLifecycle::Issued
        || inspection.in_flight_operations != 0
    {
        return Err(SelectedCallRefusal::Possession);
    }
    let scope = &inspection.scope;
    if scope.host_id != claim.host_id
        || scope.boot_id != claim.boot_id
        || scope.plan_id != claim.plan_id
        || scope.active_play_id != claim.active_play_id
        || scope.base_instance_id != claim.base_instance_id
        || scope.base_provider_generation != claim.base_provider_generation
        || scope.implementation_id != claim.implementation_id
        || scope.operation_contract_id != claim.operation_contract_id
        || scope.subject_kind != claim.subject_kind
        || scope.resource_pool_id != claim.resource_pool_id
        || scope.resource_generation_id != claim.resource_generation_id
        || scope.envelope_id != claim.envelope_id
        || gear.capability_id != scope.capability_id
        || !gear.authority.iter().any(|authority| {
            authority.grant_id == scope.authority_grant_id
                && authority.contract_id == scope.authority_contract_id
                && authority.host_call_contract_id == scope.operation_contract_id
                && authority.subject_kind == scope.subject_kind
                && authority.host_id == scope.host_id
                && authority.boot_id == scope.boot_id
                && authority.capability_id == scope.capability_id
        })
        || !gear.resources.iter().any(|resource| {
            resource.pool_id == scope.resource_pool_id
                && resource.units > 0
                && resource
                    .protected
                    .as_ref()
                    .is_none_or(|protected| protected.maximum_bytes >= definition.resource_bytes)
        })
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let [required] = gear.host_calls.as_slice() else {
        return Err(SelectedCallRefusal::WrongBinding);
    };
    if required.contract_id.as_str() != definition.call
        || required.target_kind.as_ref() != Some(&expected.kind_id)
        || required.maximum_in_flight != 1
        || required.maximum_input_bytes != definition.input_bytes
        || required.maximum_output_bytes != definition.output_bytes
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let mut nodes = lowered
        .nodes
        .iter()
        .filter(|node| &node.placement_id == placement_id);
    let node = nodes.next().ok_or(SelectedCallRefusal::WrongBinding)?;
    if nodes.next().is_some()
        || !lowered
            .identity
            .placements
            .contains(&(node.node, placement_id.clone()))
        || node.inputs.len() != 1
        || node.outputs.len() != 1
        || node.inputs[0].port != PortId(0)
        || node.outputs[0].port != PortId(0)
        || node.inputs[0].value_kind != expected.inputs[0].value_kind
        || node.outputs[0].value_kind != expected.outputs[0].value_kind
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let mut calls = lowered
        .host_calls
        .iter()
        .filter(|call| call.node == node.node);
    let call = calls.next().ok_or(SelectedCallRefusal::WrongBinding)?;
    if calls.next().is_some()
        || call.call != HostCallId(0)
        || call.binding.call != call.call
        || call.contract_id != required.contract_id
        || call.target_kind != required.target_kind
        || call.maximum_in_flight != 1
        || call.binding.maximum_input_bytes != definition.input_bytes
        || call.binding.maximum_output_bytes != definition.output_bytes
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    Ok(node.node)
}
