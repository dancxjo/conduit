//! Preparation-only assigned service routing over original opaque operations.
//! Success proves NO reservation or physical custody. Original Ready owners
//! must validate providers/original grants; complete live resource admission and
//! the ordinary Start ledger check remain mandatory before their transfer.
use crate::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_operation::{AssignedOperationInput, SelectedAssignedOperationPlan},
    selected_operation_set::{
        SelectedOperationSet, SelectedOperationSetRefusal as E, SelectedServiceContract,
        SelectedServiceRoute, MAXIMUM_SELECTED_OPERATIONS,
    },
};
use conduit_core as c;
use conduit_kernel::{HostCallId, PortId};
#[path = "assigned_configuration.rs"]
mod configuration_check;

/// Independently retained native/implementation facts, not received IDs.
/// Each authority corresponds to the trusted call and actual operation order.
/// These pins cannot manufacture a provider, original grant, or reservation.
pub struct AssignedServiceNativeIdentity<'a> {
    pub capability: &'a str,
    pub execution_profile: &'a str,
    pub artifact: &'a str,
    pub base: &'a str,
    pub base_implementation: &'a str,
    pub mechanism: &'a str,
    pub resource_class: &'a str,
    pub authorities: &'a [&'a str],
}
pub struct SelectedAssignedServicePlan<'a> {
    pub operation: SelectedAssignedOperationPlan<'a>,
    pub fore: Option<&'a [c::AssignedForeRequirement]>,
    /// Independently retained numeric parent index. Allow-list membership and
    /// matching records do not prove this resource has actually been reserved.
    pub resource: u16,
}

/// Check tables before admission. Returns routing metadata only; it is not an
/// execution ticket or native/resource possession. Allocates no retained tables.
pub fn check_selected_assigned_operation_set(
    operations: &[&c::BoundBaseOperation],
    definition: SelectedServiceContract<'_>,
    native: AssignedServiceNativeIdentity<'_>,
    selected: SelectedAssignedServicePlan<'_>,
) -> Result<SelectedOperationSet, E> {
    let plan = selected.operation;
    let count = definition.calls.len();
    if count == 0
        || count > MAXIMUM_SELECTED_OPERATIONS
        || operations.len() != count
        || native.authorities.len() != count
    {
        return Err(E::Bounds);
    }
    let kind = definition.kind;
    if kind.validate().is_err()
        || kind.inputs.len() != count
        || kind.outputs.len() != count
        || definition.resource.units != 1
        || definition.resource.protected.is_some()
        || definition.resource.compute.is_some()
        || definition.resource.content.is_some()
        || definition.resource.class_id.as_str() != native.resource_class
        || definition.requirement.class_id != definition.resource.class_id
        || definition.requirement.units != 1
        || definition.requirement.protected_role.is_some()
        || definition.requirement.compute.is_some()
        || definition.requirement.content.is_some()
        || !plan.requirements.resources.contains(&selected.resource)
    {
        return Err(E::WrongBinding);
    }
    let schema = if selected.fore.is_some() {
        c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA
    } else {
        c::ASSIGNED_NATIVE_PLAN_SCHEMA
    };
    if plan.input.header().get(8..10) != Some(schema.to_le_bytes().as_slice()) {
        return Err(E::WrongBinding);
    }
    let view = plan
        .input
        .decode(
            plan.maxima,
            plan.requirements,
            plan.computations,
            selected.fore,
        )
        .map_err(|_| E::WrongBinding)?;
    let id = c::AssignedIdentity::from_text;
    if view.fragment != id(plan.fragment_id) || !plan.scope.matches(&view, plan.input.header()) {
        return Err(E::WrongBinding);
    }
    configuration_check::check(plan.input, plan.node, definition.configuration)
        .map_err(|_| E::WrongBinding)?;
    let first = &operations[0].inspection().scope;
    let mut routes = [None; MAXIMUM_SELECTED_OPERATIONS];
    for (index, (call, operation)) in definition.calls.iter().zip(operations).enumerate() {
        if definition.calls[..index].iter().any(|prior| {
            prior.contract == call.contract
                || prior.input_port == call.input_port
                || prior.output_port == call.output_port
        }) || operations[..index].iter().any(|prior| {
            prior.inspection().scope.authority_grant_id
                == operation.inspection().scope.authority_grant_id
        }) {
            return Err(E::WrongBinding);
        }
        let inspection = operation.inspection();
        let s = &inspection.scope;
        if inspection.lifecycle != c::CapabilityLifecycle::Issued
            || inspection.in_flight_operations != 0
            || inspection.completed_operations != 0
            || s.maximum_operations == 0
        {
            return Err(E::Possession);
        }
        if !scope_matches(plan.scope, s)
            || id(s.host_id.as_str()) != view.host
            || id(s.boot_id.as_str()) != view.boot
            || s.capability_id.as_str() != native.capability
            || s.authority_contract_id.as_str() != native.authorities[index]
            || s.host_id != first.host_id
            || s.boot_id != first.boot_id
            || s.plan_id != first.plan_id
            || s.active_play_id != first.active_play_id
            || s.capability_id != first.capability_id
            || s.implementation_id != first.implementation_id
            || s.base_instance_id != first.base_instance_id
            || s.base_provider_generation != first.base_provider_generation
            || s.resource_pool_id != definition.resource.pool_id
            || &s.resource_generation_id != definition.resource_generation
            || &s.envelope_id != definition.envelope
            || &s.operation_contract_id != call.contract
            || &s.subject_kind != call.target_kind
        {
            return Err(E::WrongBinding);
        }
        if call.input_bytes == 0
            || call.output_bytes == 0
            || call.work_units == 0
            || call.maximum_operations == 0
            || operation.bound_parameter_bytes() != call.input_bytes
            || operation.bound_work_units() != call.work_units
            || s.maximum_parameter_bytes != call.input_bytes
            || s.maximum_result_bytes != call.output_bytes
            || s.maximum_work_units != call.work_units
            || s.maximum_operations != call.maximum_operations
            || s.maximum_in_flight != 1
        {
            return Err(E::Bounds);
        }
        let input = kind
            .inputs
            .iter()
            .position(|p| &p.port_id == call.input_port)
            .ok_or(E::WrongBinding)?;
        let output = kind
            .outputs
            .iter()
            .position(|p| &p.port_id == call.output_port)
            .ok_or(E::WrongBinding)?;
        routes[index] = Some(SelectedServiceRoute {
            call: HostCallId(0), // Filled from the exact independently validated call record below.
            input: PortId(input as u16),
            output: PortId(output as u16),
            maximum_input_bytes: call.input_bytes,
            maximum_output_bytes: call.output_bytes,
        });
    }
    let mut nodes = 0;
    let mut resources = 0;
    let mut ports = [[false; MAXIMUM_SELECTED_OPERATIONS]; 2];
    let mut calls = [false; MAXIMUM_SELECTED_OPERATIONS];
    for record in plan.input.records().map_err(|_| E::WrongBinding)? {
        let record = record.map_err(|_| E::WrongBinding)?;
        let d = record.payload;
        if !matches!(
            record.tag,
            c::ASSIGNED_NODE | c::ASSIGNED_PORT | c::ASSIGNED_HOST_CALL | c::ASSIGNED_RESOURCE
        ) || u16_at(d, 0)? != plan.node.0
        {
            continue;
        }
        match record.tag {
            c::ASSIGNED_NODE => {
                nodes += 1;
                if identity(d, 4)? != id(kind.kind_id.as_str())
                    || identity(d, 20)? != id(first.implementation_id.as_str())
                    || identity(d, 36)? != id(native.artifact)
                {
                    return Err(E::WrongBinding);
                }
            }
            c::ASSIGNED_PORT => {
                let direction = *d.get(4).ok_or(E::WrongBinding)? as usize;
                let ordinal = usize::from(u16_at(d, 2)?);
                if direction > 1 || ordinal >= count || ports[direction][ordinal] {
                    return Err(E::WrongBinding);
                }
                ports[direction][ordinal] = true;
                let expected = if direction == 0 {
                    &kind.inputs[ordinal]
                } else {
                    &kind.outputs[ordinal]
                };
                if identity(d, 5)? != id(expected.port_id.as_str())
                    || identity(d, 21)? != id(expected.value_kind.as_str())
                {
                    return Err(E::WrongBinding);
                }
            }
            c::ASSIGNED_HOST_CALL => {
                let numeric = HostCallId(u16_at(d, 2)?);
                let index = definition
                    .calls
                    .iter()
                    .position(|call| identity(d, 4).ok() == Some(id(call.contract.as_str())))
                    .ok_or(E::WrongBinding)?;
                if usize::from(numeric.0) >= count
                    || routes[..count].iter().enumerate().any(|(i, route)| {
                        calls[i] && route.is_some_and(|route| route.call == numeric)
                    })
                {
                    return Err(E::WrongBinding);
                }
                if calls[index] {
                    return Err(E::WrongBinding);
                }
                calls[index] = true;
                routes[index].as_mut().ok_or(E::WrongBinding)?.call = numeric;
                let call = &definition.calls[index];
                let s = &operations[index].inspection().scope;
                if identity(d, 4)? != id(call.contract.as_str())
                    || identity(d, 20)? != id(call.target_kind.as_str())
                    || u16_at(d, 36)? != 1
                    || u32_at(d, 38)? != call.input_bytes
                    || u32_at(d, 42)? != call.output_bytes
                {
                    return Err(E::WrongBinding);
                }
                let b = c::assigned_native_binding(d)
                    .map_err(|_| E::WrongBinding)?
                    .ok_or(E::WrongBinding)?;
                if b.capability != id(native.capability)
                    || b.kind_revision != id(kind.kind_contract_revision.as_str())
                    || b.execution_profile != id(native.execution_profile)
                    || b.maximum_instances != kind.limits.max_active_instances
                    || b.queue_items != kind.limits.max_queue_items
                    || b.queue_bytes != kind.limits.max_queue_bytes
                    || b.base != id(native.base)
                    || b.instance != id(s.base_instance_id.as_str())
                    || b.generation != s.base_provider_generation
                    || b.base_implementation != id(native.base_implementation)
                    || b.mechanism != id(native.mechanism)
                    || b.enforcement != 0
                    || b.grant != id(s.authority_grant_id.as_str())
                    || b.authority != id(native.authorities[index])
                    || b.call != id(call.contract.as_str())
                    || b.subject != id(call.target_kind.as_str())
                    || b.host != view.host
                    || b.boot != view.boot
                    || b.authority_capability != b.capability
                    || b.pool != id(s.resource_pool_id.as_str())
                    || b.resource_class != id(native.resource_class)
                    || b.resource != selected.resource
                    || b.units != 1
                {
                    return Err(E::WrongBinding);
                }
            }
            c::ASSIGNED_RESOURCE => {
                resources += 1;
                if u16_at(d, 2)? != selected.resource || u32_at(d, 4)? != 1 {
                    return Err(E::WrongBinding);
                }
            }
            _ => {}
        }
    }
    if nodes != 1
        || resources != 1
        || ports.iter().any(|p| p[..count].iter().any(|seen| !seen))
        || calls[..count].iter().any(|seen| !seen)
    {
        return Err(E::WrongBinding);
    }
    Ok(SelectedOperationSet {
        node: plan.node,
        routes,
    })
}
fn scope_matches(scope: AssignedPlayScope<'_>, s: &c::BaseCapabilityScope) -> bool {
    match scope {
        AssignedPlayScope::Plot(active) => {
            s.plan_id == active.plan_id
                && s.active_play_id == active.active_play_id
                && s.host_id == active.host_id
                && s.boot_id == active.boot_id
        }
        #[cfg(feature = "delegated-preparation")]
        AssignedPlayScope::Delegated(scope) => {
            let m = scope.metadata();
            s.plan_id.as_str() == m.body_plan
                && s.active_play_id.as_str() == m.body_play
                && s.host_id.as_str() == m.host
                && s.boot_id.as_str() == m.boot
        }
        #[cfg(feature = "body-plan-binding")]
        AssignedPlayScope::Body(scope) => {
            s.plan_id == scope.plan().plan_id
                && s.active_play_id == scope.play().active_play_id
                && s.host_id == scope.fragment().host_id
                && s.boot_id == scope.fragment().boot_id
        }
    }
}
fn u16_at(d: &[u8], o: usize) -> Result<u16, E> {
    Ok(u16::from_le_bytes(
        d.get(o..o + 2)
            .ok_or(E::WrongBinding)?
            .try_into()
            .map_err(|_| E::WrongBinding)?,
    ))
}
fn u32_at(d: &[u8], o: usize) -> Result<u32, E> {
    Ok(u32::from_le_bytes(
        d.get(o..o + 4)
            .ok_or(E::WrongBinding)?
            .try_into()
            .map_err(|_| E::WrongBinding)?,
    ))
}
fn identity(d: &[u8], o: usize) -> Result<c::AssignedIdentity, E> {
    Ok(c::AssignedIdentity(
        d.get(o..o + 16)
            .ok_or(E::WrongBinding)?
            .try_into()
            .map_err(|_| E::WrongBinding)?,
    ))
}
