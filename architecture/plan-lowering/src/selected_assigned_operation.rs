//! Exact single cooperative native operation from an explicitly admitted assigned
//! projection. Serialized facts do not replace a locally issued opaque capability.
use crate::selected_operation::{
    matching_scope, SelectedCallRefusal as E, SelectedOperationContract,
};
use conduit_core as c;
use conduit_kernel::{NodeId, PortId};

pub struct SelectedAssignedOperationPlan<'a> {
    pub bytes: &'a [u8],
    pub maxima: c::AssignedPlanMaxima,
    pub requirements: c::AssignedPlanRequirements<'a>,
    pub computations: c::AssignedComputationProfile<'a>,
    pub fragment_id: &'a c::FragmentId,
    pub active: &'a c::ActivePlayIdentity,
    pub node: NodeId,
}
/// Actual implementation/attachment facts retained independently by the native
/// owner. None may be inferred from an incoming assignment or successful probe.
pub struct AssignedNativeIdentity<'a> {
    pub capability: &'a str,
    pub execution_profile: &'a str,
    pub artifact: &'a str,
    pub base: &'a str,
    pub base_implementation: &'a str,
    pub mechanism: &'a str,
    pub authority: &'a str,
    pub resource_class: &'a str,
}

pub fn bind_selected_assigned_operation(
    table: &c::BaseCapabilityTable,
    claim: &c::BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    native: AssignedNativeIdentity<'_>,
    selected: SelectedAssignedOperationPlan<'_>,
) -> Result<NodeId, E> {
    let wrong = || E::WrongBinding;
    let id = c::AssignedIdentity::from_text;
    if selected.bytes.get(8..10) != Some(c::ASSIGNED_NATIVE_PLAN_SCHEMA.to_le_bytes().as_slice()) {
        return Err(wrong());
    }
    let view = c::decode_assigned_plan_with_computations(
        selected.bytes,
        selected.maxima,
        selected.requirements,
        selected.computations,
    )
    .map_err(|_| wrong())?;
    let active = selected.active;
    if view.plan != id(claim.plan_id.as_str())
        || view.fragment != id(selected.fragment_id.as_str())
        || view.host != id(claim.host_id.as_str())
        || view.boot != id(claim.boot_id.as_str())
        || active.plan_id != claim.plan_id
        || active.host_id != claim.host_id
        || active.boot_id != claim.boot_id
        || active.active_play_id != claim.active_play_id
        || c::bind_active_play(
            &claim.plan_id,
            &claim.host_id,
            &claim.boot_id,
            active.play_sequence,
        ) != *active
        || claim.subject_kind != definition.kind.kind_id
        || claim.operation_contract_id.as_str() != definition.call
    {
        return Err(wrong());
    }
    let scope = matching_scope(table, claim)?;
    if scope.capability_id.as_str() != native.capability
        || scope.authority_contract_id.as_str() != native.authority
    {
        return Err(wrong());
    }
    let [input] = definition.kind.inputs.as_slice() else {
        return Err(wrong());
    };
    let [output] = definition.kind.outputs.as_slice() else {
        return Err(wrong());
    };
    let mut nodes = 0;
    let mut inputs = 0;
    let mut outputs = 0;
    let mut calls = 0;
    let mut resources = 0;
    let mut binding = None;
    for record in c::AssignedPlanRecords::new(selected.bytes).map_err(|_| wrong())? {
        let record = record.map_err(|_| wrong())?;
        let data = record.payload;
        if !matches!(
            record.tag,
            c::ASSIGNED_NODE | c::ASSIGNED_PORT | c::ASSIGNED_HOST_CALL | c::ASSIGNED_RESOURCE
        ) {
            continue;
        }
        if u16_at(data, 0)? != selected.node.0 {
            continue;
        }
        match record.tag {
            c::ASSIGNED_NODE => {
                nodes += 1;
                if identity(data, 4)? != id(definition.kind.kind_id.as_str())
                    || identity(data, 20)? != id(claim.implementation_id.as_str())
                    || identity(data, 36)? != id(native.artifact)
                {
                    return Err(wrong());
                }
            }
            c::ASSIGNED_PORT => {
                let expected = match data[4] {
                    0 => {
                        inputs += 1;
                        input
                    }
                    1 => {
                        outputs += 1;
                        output
                    }
                    _ => return Err(wrong()),
                };
                if u16_at(data, 2)? != PortId(0).0
                    || identity(data, 5)? != id(expected.port_id.as_str())
                    || identity(data, 21)? != id(expected.value_kind.as_str())
                {
                    return Err(wrong());
                }
            }
            c::ASSIGNED_HOST_CALL => {
                calls += 1;
                if u16_at(data, 2)? != 0
                    || identity(data, 4)? != id(definition.call)
                    || identity(data, 20)? != id(definition.kind.kind_id.as_str())
                    || u16_at(data, 36)? != 1
                    || u32_at(data, 38)? != definition.input_bytes
                    || u32_at(data, 42)? != definition.output_bytes
                {
                    return Err(wrong());
                }
                binding = c::assigned_native_binding(data).map_err(|_| wrong())?;
            }
            c::ASSIGNED_RESOURCE => {
                resources += 1;
            }
            _ => {}
        }
    }
    if (nodes, inputs, outputs, calls, resources) != (1, 1, 1, 1, 1) {
        return Err(wrong());
    }
    let b = binding.ok_or_else(wrong)?;
    let limits = &definition.kind.limits;
    if b.capability != id(native.capability)
        || b.kind_revision != id(definition.kind.kind_contract_revision.as_str())
        || b.execution_profile != id(native.execution_profile)
        || b.maximum_instances != limits.max_active_instances
        || b.queue_items != limits.max_queue_items
        || b.queue_bytes != limits.max_queue_bytes
        || b.base != id(native.base)
        || b.instance != id(claim.base_instance_id.as_str())
        || b.generation != claim.base_provider_generation
        || b.base_implementation != id(native.base_implementation)
        || b.mechanism != id(native.mechanism)
        || b.enforcement != 0
        || b.grant != id(scope.authority_grant_id.as_str())
        || b.authority != id(native.authority)
        || b.call != id(definition.call)
        || b.subject != id(definition.kind.kind_id.as_str())
        || b.host != view.host
        || b.boot != view.boot
        || b.authority_capability != b.capability
        || b.pool != id(claim.resource_pool_id.as_str())
        || b.resource_class != id(native.resource_class)
        || b.units != 1
    {
        return Err(wrong());
    }
    for record in c::AssignedPlanRecords::new(selected.bytes).map_err(|_| wrong())? {
        let record = record.map_err(|_| wrong())?;
        if record.tag == c::ASSIGNED_RESOURCE
            && u16_at(record.payload, 0)? == selected.node.0
            && (u16_at(record.payload, 2)? != b.resource || u32_at(record.payload, 4)? != b.units)
        {
            return Err(wrong());
        }
    }
    Ok(selected.node)
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
