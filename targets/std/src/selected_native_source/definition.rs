//! Validate independently initialized Source contracts before registration.
use super::*;

pub(super) fn validate_definition(
    d: &NativeSourceDefinition,
    host: &HostAdvertisement,
    worker_bytes: Option<u32>,
) -> Result<(), String> {
    d.kind
        .validate()
        .map_err(|error| format!("native Source Kind: {error:?}"))?;
    if d.base.lifecycle != BaseLifecycle::Ready
        || d.base.capabilities.len() != 1
        || d.base.resources.len() != if worker_bytes.is_some() { 2 } else { 1 }
        || d.base.provider_generation == 0
        || d.envelope_id.as_str().is_empty()
        || d.resource_generation.0.is_empty()
        || d.input_bytes == 0
        || d.output_bytes == 0
        || d.input_bytes.checked_add(d.output_bytes).is_none()
        || d.resource_bytes == 0
        || d.kind.inputs.len() != 1
        || d.kind.outputs.len() != 1
    {
        return Err("native Source requires one finite ready provider, call and resource".into());
    }
    if let Some(required) = worker_bytes {
        let memory = &d.base.resources[1];
        if memory.class_id.as_str() != RUNTIME_MEMORY_RESOURCE_CLASS
            || memory.capacity_units < required
            || memory.pool_id == d.base.resources[0].pool_id
            || memory.compute.is_some()
            || memory.content.is_some()
        {
            return Err("native worker's independently configured backing pool is insufficient or mismatched".into());
        }
    }
    let offer = &d.base.capabilities[0];
    let grant = &d.grant;
    if offer.capability_id.as_str().is_empty()
        || offer
            .implementation
            .execution_profile_id
            .as_str()
            .is_empty()
        || offer.implementation.implementation_id.as_str().is_empty()
        || offer.implementation.artifact_id.as_str().is_empty()
        || grant.grant_id.as_str().is_empty()
        || offer.startup_parameters != d.kind.startup_parameters
        || offer.shorthand != d.kind.shorthand
        || offer.limits != d.kind.limits
        || offer.limits.max_active_instances != 1
        || offer.kind_id != d.kind.kind_id
        || offer.kind_contract_revision != d.kind.kind_contract_revision
        || offer.inputs != d.kind.inputs
        || offer.outputs != d.kind.outputs
        || offer.semantic_contract != d.kind.semantic_contract()
        || offer.host_calls.len() != 1
        || offer.resource_requirements.len() != 1
        || offer.authority_requirements.len() != 1
        || offer.host_calls[0].maximum_input_bytes != d.input_bytes
        || offer.host_calls[0].maximum_output_bytes != d.output_bytes
        || offer.host_calls[0].maximum_in_flight != 1
        || offer.host_calls[0].target_kind.as_ref() != Some(&d.kind.kind_id)
        || grant.host_id != host.host_id
        || grant.boot_id != host.boot_id
        || grant.capability_id != offer.capability_id
        || grant.subject_kind != d.kind.kind_id
        || grant.host_call_contract_id != offer.host_calls[0].contract_id
        || grant.contract_id != offer.authority_requirements[0].contract_id
        || grant.host_call_contract_id != offer.authority_requirements[0].host_call_contract_id
        || grant.subject_kind != offer.authority_requirements[0].subject_kind
        || offer.resource_requirements[0].units != 1
        || d.base.resources[0].capacity_units != 1
        || d.base.resources[0].class_id != offer.resource_requirements[0].class_id
    {
        return Err("native Source definition, grant and initialized Base disagree".into());
    }
    Ok(())
}
