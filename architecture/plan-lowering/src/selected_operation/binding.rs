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
    let mut matches = fragment
        .placements
        .iter()
        .filter(|gear| &gear.placement_id == placement_id);
    let gear = matches.next().ok_or(SelectedCallRefusal::WrongBinding)?;
    if matches.next().is_some() {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let semantic_contract = definition.kind.semantic_contract();
    let definition = SelectedOperationContractRef {
        kind: definition.kind.into(),
        semantic_contract: &semantic_contract,
        call: definition.call,
        input_bytes: definition.input_bytes,
        output_bytes: definition.output_bytes,
        resource_bytes: definition.resource_bytes,
    };
    checked_gear_ref(
        table,
        claim,
        &definition,
        subject.as_str(),
        gear.into(),
        |_| configuration_matches(gear),
    )?;
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

// Shared complete field validation; rich selection keeps its ordered arrays.
pub(super) fn checked_gear_ref<'a>(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: &SelectedOperationContractRef<'_>,
    subject: &str,
    gear: conduit_core::PlannedGearRef<'a>,
    configuration_matches: impl FnOnce(conduit_core::PlannedGearRef<'a>) -> bool,
) -> Result<conduit_core::PlannedGearRef<'a>, SelectedCallRefusal> {
    checked_gear_native_ref(
        NativeScopeRef::claim(claim),
        definition,
        subject,
        gear,
        configuration_matches,
    )?;
    checked_gear_grant_ref(matching_scope(table, claim)?, definition, gear)
}

/// Descriptive half of binding. This checks an exact independently supplied
/// expected scope against immutable verified placement data; it grants nothing.
pub(super) fn checked_gear_scope_ref<'a>(
    scope: &conduit_core::BaseCapabilityScope,
    definition: &SelectedOperationContractRef<'_>,
    subject: &str,
    gear: conduit_core::PlannedGearRef<'a>,
    configuration_matches: impl FnOnce(conduit_core::PlannedGearRef<'a>) -> bool,
) -> Result<conduit_core::PlannedGearRef<'a>, SelectedCallRefusal> {
    checked_gear_native_ref(
        NativeScopeRef::scope(scope),
        definition,
        subject,
        gear,
        configuration_matches,
    )?;
    checked_gear_grant_ref(scope, definition, gear)
}

struct NativeScopeRef<'a> {
    host: &'a str,
    boot: &'a str,
    implementation: &'a str,
    instance: &'a str,
    generation: u64,
    subject: &'a str,
    call: &'a str,
}
impl<'a> NativeScopeRef<'a> {
    fn claim(scope: &'a BaseOperationClaim) -> Self {
        Self {
            host: scope.host_id.as_str(),
            boot: scope.boot_id.as_str(),
            implementation: scope.implementation_id.as_str(),
            instance: scope.base_instance_id.as_str(),
            generation: scope.base_provider_generation,
            subject: scope.subject_kind.as_str(),
            call: scope.operation_contract_id.as_str(),
        }
    }
    fn scope(scope: &'a conduit_core::BaseCapabilityScope) -> Self {
        Self {
            host: scope.host_id.as_str(),
            boot: scope.boot_id.as_str(),
            implementation: scope.implementation_id.as_str(),
            instance: scope.base_instance_id.as_str(),
            generation: scope.base_provider_generation,
            subject: scope.subject_kind.as_str(),
            call: scope.operation_contract_id.as_str(),
        }
    }
}
fn checked_gear_native_ref<'a>(
    scope: NativeScopeRef<'_>,
    definition: &SelectedOperationContractRef<'_>,
    subject: &str,
    gear: conduit_core::PlannedGearRef<'a>,
    configuration_matches: impl FnOnce(conduit_core::PlannedGearRef<'a>) -> bool,
) -> Result<(), SelectedCallRefusal> {
    let expected = definition.kind;
    let base = gear.base.ok_or(SelectedCallRefusal::WrongBinding)?;
    if gear.kind_id != expected.kind_id
        || gear.kind_contract_revision != expected.kind_contract_revision
        || !gear.inputs.iter().eq(expected.inputs.iter())
        || !gear.outputs.iter().eq(expected.outputs.iter())
        || gear.semantic_contract != definition.semantic_contract
        || !configuration_matches(gear)
        || gear.host_id != scope.host
        || gear.boot_id != scope.boot
        || gear.implementation_id != scope.implementation
        || base.provider_instance_id != scope.instance
        || base.provider_generation != scope.generation
        || scope.subject != subject
        || scope.call != definition.call
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    Ok(())
}
fn checked_gear_grant_ref<'a>(
    scope: &conduit_core::BaseCapabilityScope,
    definition: &SelectedOperationContractRef<'_>,
    gear: conduit_core::PlannedGearRef<'a>,
) -> Result<conduit_core::PlannedGearRef<'a>, SelectedCallRefusal> {
    if gear.capability_id != scope.capability_id.as_str()
        || !gear.authority.iter().any(|authority| {
            authority.grant_id == scope.authority_grant_id.as_str()
                && authority.contract_id == scope.authority_contract_id.as_str()
                && authority.host_call_contract_id == scope.operation_contract_id.as_str()
                && authority.subject_kind == scope.subject_kind.as_str()
                && authority.host_id == scope.host_id.as_str()
                && authority.boot_id == scope.boot_id.as_str()
                && authority.capability_id == scope.capability_id.as_str()
        })
        || !gear.resources.iter().any(|resource| {
            resource.pool_id == scope.resource_pool_id.as_str()
                && resource.units > 0
                && resource
                    .protected
                    .as_ref()
                    .is_none_or(|protected| protected.maximum_bytes >= definition.resource_bytes)
        })
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    if gear.host_calls.len() != 1 {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let required = gear
        .host_calls
        .iter()
        .next()
        .ok_or(SelectedCallRefusal::WrongBinding)?;
    if required.contract_id != definition.call
        || required.target_kind != Some(scope.subject_kind.as_str())
        || required.maximum_in_flight != 1
        || required.maximum_input_bytes != definition.input_bytes
        || required.maximum_output_bytes != definition.output_bytes
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    Ok(gear)
}
