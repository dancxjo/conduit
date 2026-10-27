//! Historical canonical placement fields, shared by complete and incremental hashing.
use super::*;
pub(super) fn push_placement(canonical: &mut impl CanonicalSink, gear: crate::PlannedGearRef<'_>) {
    push_string(canonical, gear.placement_id);
    push_string(canonical, gear.gear_id);
    push_string(canonical, gear.kind_id);
    push_string(canonical, gear.kind_contract_revision);
    push_string(canonical, gear.execution_profile_id);
    crate::plan_fingerprint::push_configuration(canonical, gear.configuration);
    push_string(canonical, gear.host_id);
    push_string(canonical, gear.boot_id);
    push_u64(canonical, gear.offer_generation.0);
    push_string(canonical, gear.capability_id);
    push_string(canonical, gear.implementation_id);
    push_string(canonical, gear.artifact_id);
    if let Some(span) = gear.source_span {
        push_string(canonical, "source-span@1");
        push_u64(canonical, span.start);
        push_u64(canonical, span.end);
        push_u64(canonical, span.line);
        push_u64(canonical, span.column);
        push_u64(canonical, span.end_line);
        push_u64(canonical, span.end_column);
    }
    if let Some(base) = &gear.base {
        push_string(canonical, "base-provider-binding@1");
        push_string(canonical, base.base_id);
        push_string(canonical, base.provider_instance_id);
        push_u64(canonical, base.provider_generation);
        push_string(canonical, base.implementation_id);
        push_string(canonical, base.mechanism_family);
        canonical.push(base.enforcement_class as u8);
    }
    push_u32(canonical, gear.realization_characteristics.len() as u32);
    for characteristic in gear.realization_characteristics {
        characteristic::push_characteristic_canonical(canonical, characteristic);
    }
    if !gear.realization_properties.is_empty() {
        crate::realization_properties::push_canonical(canonical, gear.realization_properties);
    }
    canonical.extend_from_slice(&gear.limits.max_active_instances.to_le_bytes());
    canonical.extend_from_slice(&gear.limits.max_queue_items.to_le_bytes());
    push_u32(canonical, gear.limits.max_queue_bytes);
    push_ports(canonical, gear.inputs);
    push_ports(canonical, gear.outputs);
    push_semantic_contract(canonical, gear.semantic_contract);
    push_u32(canonical, gear.terminal_transductions.len() as u32);
    for profile in gear.terminal_transductions {
        push_string(canonical, "terminal-transduction@2");
        push_terminal_transduction(canonical, profile);
    }
    push_u32(canonical, gear.host_calls.len() as u32);
    for requirement in gear.host_calls {
        push_string(canonical, requirement.contract_id.as_str());
        match &requirement.target_kind {
            Some(target_kind) => {
                canonical.push(1);
                push_string(canonical, target_kind.as_str());
            }
            None => canonical.push(0),
        }
        canonical.extend_from_slice(&requirement.maximum_in_flight.to_le_bytes());
        push_u32(canonical, requirement.maximum_input_bytes);
        push_u32(canonical, requirement.maximum_output_bytes);
    }
    push_u32(canonical, gear.resources.len() as u32);
    for binding in gear.resources.iter() {
        crate::resource_canonical::push_resource_binding_ref(canonical, binding);
    }
    push_u32(canonical, gear.authority.len() as u32);
    for binding in gear.authority.iter() {
        push_string(canonical, binding.grant_id);
        push_string(canonical, binding.contract_id);
        push_string(canonical, binding.host_call_contract_id);
        push_string(canonical, binding.subject_kind);
        push_string(canonical, binding.host_id);
        push_string(canonical, binding.boot_id);
        push_string(canonical, binding.capability_id);
    }
    push_u32(canonical, gear.pool_references.len() as u32);
    for pool in gear.pool_references {
        push_string(canonical, pool.as_str());
    }
}

#[cfg(test)]
mod tests;
