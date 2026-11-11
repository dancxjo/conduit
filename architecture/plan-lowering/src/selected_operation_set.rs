//! Preparation-only routing for a finite service over actual opaque operations.
//! This validates one placement/reservation, not physical provider custody or
//! safe device windows. Returned numeric metadata confers no execution authority;
//! callers retain the original operations and physical owner and must authorize
//! every actual effect. No grant, bearer, table, or reservation is duplicated.
use crate::{lowering::lower_plan_fragment, selected_operation::SelectedOperationPlan};
use conduit_core as c;
use conduit_kernel::{HostCallId, NodeId, PortId};

pub const MAXIMUM_SELECTED_OPERATIONS: usize = 4;

/// Independently retained trusted implementation contract, not incoming Plan
/// assertions. Bounds are exact finite scopes, including total operation count.
pub struct SelectedServiceCall<'a> {
    pub contract: &'a c::HostCallContractId,
    pub target_kind: &'a c::KindId,
    pub input_port: &'a c::PortId,
    pub output_port: &'a c::PortId,
    pub input_bytes: u32,
    pub output_bytes: u32,
    pub work_units: u64,
    pub maximum_operations: u32,
}
pub struct SelectedServiceContract<'a> {
    pub kind: &'a c::Kind,
    pub configuration: &'a [c::ConfigurationEntry],
    pub resource: &'a c::ResourceBinding,
    pub requirement: &'a c::ResourceRequirement,
    pub resource_generation: &'a c::ResourceGenerationId,
    pub envelope: &'a c::CapabilityEnvelopeId,
    pub calls: &'a [SelectedServiceCall<'a>],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedServiceRoute {
    pub call: HostCallId,
    pub input: PortId,
    pub output: PortId,
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedOperationSet {
    pub node: NodeId,
    /// Entries correspond to trusted contract/opaque-operation order, while
    /// call and port IDs come from ordinary lowering, not that slice order.
    pub routes: [Option<SelectedServiceRoute>; MAXIMUM_SELECTED_OPERATIONS],
}
#[derive(Debug, PartialEq, Eq)]
pub enum SelectedOperationSetRefusal {
    WrongBinding,
    Possession,
    Reservation,
    Bounds,
}

pub fn bind_selected_operation_set(
    operations: &[&c::BoundBaseOperation],
    definition: SelectedServiceContract<'_>,
    selection: SelectedOperationPlan<'_>,
    owner: &c::ResourceAdmissionOwner,
) -> Result<SelectedOperationSet, SelectedOperationSetRefusal> {
    use SelectedOperationSetRefusal as E;
    let SelectedOperationPlan {
        fragment,
        lowered,
        active,
        placement_id,
    } = selection;
    let count = definition.calls.len();
    if count == 0 || count > MAXIMUM_SELECTED_OPERATIONS || operations.len() != count {
        return Err(E::Bounds);
    }
    if !c::verify_plan_fragment(fragment)
        || !crate::active_scope::matches(active)
        || active.plan_id != fragment.plan_id
        || active.host_id != fragment.host_id
        || active.boot_id != fragment.boot_id
        || lower_plan_fragment(fragment).map_err(|_| E::WrongBinding)? != *lowered
    {
        return Err(E::WrongBinding);
    }
    let mut gears = fragment
        .placements
        .iter()
        .filter(|g| &g.placement_id == placement_id);
    let gear = gears.next().ok_or(E::WrongBinding)?;
    if gears.next().is_some() {
        return Err(E::WrongBinding);
    }
    let kind = definition.kind;
    if kind.validate().is_err()
        || gear.kind_id != kind.kind_id
        || gear.kind_contract_revision != kind.kind_contract_revision
        || gear.inputs != kind.inputs
        || gear.outputs != kind.outputs
        || gear.semantic_contract != kind.semantic_contract()
        || gear.configuration != definition.configuration
        || gear.host_id != fragment.host_id
        || gear.boot_id != fragment.boot_id
        || gear.offer_generation != fragment.offer_generation
        || gear.inputs.len() != count
        || gear.outputs.len() != count
        || gear.host_calls.len() != count
        || gear.authority.len() != count
        || gear.resources.as_slice() != core::slice::from_ref(definition.resource)
        || definition.resource.units != 1
    {
        return Err(E::WrongBinding);
    }
    let base = gear.base.as_ref().ok_or(E::WrongBinding)?;
    let host = owner.host();
    if host.host_id != fragment.host_id
        || host.boot_id != fragment.boot_id
        || host.offer_generation != fragment.offer_generation
    {
        return Err(E::Reservation);
    }
    let mut offers = host
        .capabilities
        .iter()
        .filter(|offer| offer.capability_id == gear.capability_id);
    let offer = offers.next().ok_or(E::Reservation)?;
    if offers.next().is_some()
        || offer.kind_id != gear.kind_id
        || offer.kind_contract_revision != gear.kind_contract_revision
        || offer.inputs != gear.inputs
        || offer.outputs != gear.outputs
        || offer.semantic_contract != gear.semantic_contract
        || offer.implementation.implementation_id != gear.implementation_id
        || offer.host_calls != gear.host_calls
        || offer.resource_requirements.as_slice() != core::slice::from_ref(definition.requirement)
    {
        return Err(E::Reservation);
    }
    let mut reservations = owner
        .admissions()
        .iter()
        .filter(|a| a.plan_id == fragment.plan_id && &a.placement_id == placement_id);
    let reservation = reservations.next().ok_or(E::Reservation)?;
    if reservations.next().is_some()
        || reservation.host_id != host.host_id
        || reservation.boot_id != host.boot_id
        || reservation.offer_generation != host.offer_generation
        || reservation.items.len() != 1
        || reservation.items[0].binding != *definition.resource
        || reservation.items[0].requirement != *definition.requirement
    {
        return Err(E::Reservation);
    }
    let mut nodes = lowered
        .nodes
        .iter()
        .filter(|n| &n.placement_id == placement_id);
    let node = nodes.next().ok_or(E::WrongBinding)?;
    if nodes.next().is_some()
        || node.inputs.len() != count
        || node.outputs.len() != count
        || lowered
            .host_calls
            .iter()
            .filter(|call| call.node == node.node)
            .count()
            != count
    {
        return Err(E::WrongBinding);
    }
    let mut routes = [None; MAXIMUM_SELECTED_OPERATIONS];
    for (index, (call, operation)) in definition.calls.iter().zip(operations).enumerate() {
        if definition.calls[..index].iter().any(|previous| {
            previous.contract == call.contract
                || previous.input_port == call.input_port
                || previous.output_port == call.output_port
        }) {
            return Err(E::WrongBinding);
        }
        let inspection = operation.inspection();
        let scope = &inspection.scope;
        if inspection.lifecycle != c::CapabilityLifecycle::Issued
            || inspection.in_flight_operations != 0
            || inspection.completed_operations != 0
            || scope.maximum_operations == 0
        {
            return Err(E::Possession);
        }
        if scope.host_id != fragment.host_id
            || scope.boot_id != fragment.boot_id
            || scope.plan_id != fragment.plan_id
            || scope.active_play_id != active.active_play_id
            || scope.implementation_id != gear.implementation_id
            || scope.capability_id != gear.capability_id
            || scope.base_instance_id != base.provider_instance_id
            || scope.base_provider_generation != base.provider_generation
            || scope.resource_pool_id != definition.resource.pool_id
            || &scope.resource_generation_id != definition.resource_generation
            || &scope.envelope_id != definition.envelope
            || &scope.operation_contract_id != call.contract
            || &scope.subject_kind != call.target_kind
        {
            return Err(E::WrongBinding);
        }
        if call.input_bytes == 0
            || call.output_bytes == 0
            || call.work_units == 0
            || call.maximum_operations == 0
            || operation.bound_parameter_bytes() != call.input_bytes
            || operation.bound_work_units() != call.work_units
            || scope.maximum_parameter_bytes != call.input_bytes
            || scope.maximum_result_bytes != call.output_bytes
            || scope.maximum_work_units != call.work_units
            || scope.maximum_operations != call.maximum_operations
            || scope.maximum_in_flight != 1
        {
            return Err(E::Bounds);
        }
        if definition.calls[..index]
            .iter()
            .enumerate()
            .any(|(prior, _)| {
                operations[prior].inspection().scope.authority_grant_id == scope.authority_grant_id
            })
        {
            return Err(E::WrongBinding);
        }
        let mut authorities = gear
            .authority
            .iter()
            .filter(|a| a.grant_id == scope.authority_grant_id);
        let authority = authorities.next().ok_or(E::WrongBinding)?;
        if authorities.next().is_some()
            || authority.contract_id != scope.authority_contract_id
            || authority.host_call_contract_id != scope.operation_contract_id
            || authority.subject_kind != scope.subject_kind
            || authority.host_id != scope.host_id
            || authority.boot_id != scope.boot_id
            || authority.capability_id != scope.capability_id
        {
            return Err(E::WrongBinding);
        }
        let mut requirements = gear
            .host_calls
            .iter()
            .enumerate()
            .filter(|(_, required)| &required.contract_id == call.contract);
        let (ordinal, required) = requirements.next().ok_or(E::WrongBinding)?;
        if requirements.next().is_some()
            || required.target_kind.as_ref() != Some(call.target_kind)
            || required.maximum_input_bytes != call.input_bytes
            || required.maximum_output_bytes != call.output_bytes
            || required.maximum_in_flight != 1
        {
            return Err(E::WrongBinding);
        }
        let numeric_call = HostCallId(u16::try_from(ordinal).map_err(|_| E::Bounds)?);
        let lowered_call = lowered
            .host_calls
            .iter()
            .find(|entry| entry.node == node.node && entry.call == numeric_call)
            .ok_or(E::WrongBinding)?;
        if lowered_call.contract_id != required.contract_id
            || lowered_call.target_kind != required.target_kind
            || lowered_call.binding.call != numeric_call
            || lowered_call.maximum_in_flight != 1
            || lowered_call.binding.maximum_input_bytes != call.input_bytes
            || lowered_call.binding.maximum_output_bytes != call.output_bytes
        {
            return Err(E::WrongBinding);
        }
        let input = node
            .inputs
            .iter()
            .find(|port| &port.port_id == call.input_port)
            .ok_or(E::WrongBinding)?;
        let output = node
            .outputs
            .iter()
            .find(|port| &port.port_id == call.output_port)
            .ok_or(E::WrongBinding)?;
        routes[index] = Some(SelectedServiceRoute {
            call: numeric_call,
            input: input.port,
            output: output.port,
            maximum_input_bytes: call.input_bytes,
            maximum_output_bytes: call.output_bytes,
        });
    }
    Ok(SelectedOperationSet {
        node: node.node,
        routes,
    })
}
