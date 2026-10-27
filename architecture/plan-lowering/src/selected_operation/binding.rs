//! Common actual-possession and exact native contract checks for both representations.
use super::*;
pub(super) fn bind_fields(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    subject: &conduit_core::KindId,
    selection: SelectedOperationPlan<'_>,
    configuration_matches: impl FnOnce(&conduit_core::PlannedGear) -> bool,
) -> Result<NodeId, SelectedCallRefusal> {
    let SelectedOperationPlan {
        fragment,
        lowered,
        active: _,
        placement_id,
    } = selection;
    let gear = checked_gear(
        table,
        claim,
        &definition,
        subject,
        fragment,
        placement_id,
        configuration_matches,
    )?;
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
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let mut calls = lowered
        .host_calls
        .iter()
        .filter(|call| call.node == node.node);
    let call = calls.next().ok_or(SelectedCallRefusal::WrongBinding)?;
    if calls.next().is_some() {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    checked_numeric(gear, node, call, &definition)?;
    Ok(node.node)
}

/// Exact immutable contract, grant and current possession checks shared by rich
/// lowering and the opaque direct projection. No provider or lease is moved.
pub(super) fn checked_gear<'a>(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: &SelectedOperationContract<'_>,
    subject: &conduit_core::KindId,
    fragment: &'a PlanFragment,
    placement_id: &PlacementId,
    configuration_matches: impl FnOnce(&conduit_core::PlannedGear) -> bool,
) -> Result<&'a conduit_core::PlannedGear, SelectedCallRefusal> {
    let expected = definition.kind;
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
        || gear.semantic_contract != expected.semantic_contract()
        || !configuration_matches(gear)
        || gear.host_id != claim.host_id
        || gear.boot_id != claim.boot_id
        || gear.implementation_id != claim.implementation_id
        || base.provider_instance_id != claim.base_instance_id
        || base.provider_generation != claim.base_provider_generation
        || &claim.subject_kind != subject
        || claim.operation_contract_id.as_str() != definition.call
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let scope = matching_scope(table, claim)?;
    if gear.capability_id != scope.capability_id
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
        || required.target_kind.as_ref() != Some(subject)
        || required.maximum_in_flight != 1
        || required.maximum_input_bytes != definition.input_bytes
        || required.maximum_output_bytes != definition.output_bytes
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    Ok(gear)
}

/// Same ordinary selected numeric checks for rich and immutable direct projections.
pub(super) fn checked_numeric(
    gear: &conduit_core::PlannedGear,
    node: &crate::lowering::LoweredNode,
    call: &crate::lowering::LoweredHostCall,
    definition: &SelectedOperationContract<'_>,
) -> Result<(), SelectedCallRefusal> {
    let expected = definition.kind;
    let required = &gear.host_calls[0];
    if expected.inputs.len() > 1
        || expected.outputs.len() > 1
        || node.inputs.len() != expected.inputs.len()
        || node.outputs.len() != expected.outputs.len()
        || node
            .inputs
            .iter()
            .zip(&expected.inputs)
            .any(|(port, expected)| {
                port.port != PortId(0) || port.value_kind != expected.value_kind
            })
        || node
            .outputs
            .iter()
            .zip(&expected.outputs)
            .any(|(port, expected)| {
                port.port != PortId(0) || port.value_kind != expected.value_kind
            })
        || call.node != node.node
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
    Ok(())
}
