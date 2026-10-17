//! Compare selected Plan facts with the current installed Host/Boot offer.
use conduit_core::{resource_binding_satisfies, HostAdvertisement, PlanFragment};

pub(super) fn validate_exact_profile(
    advertisement: &HostAdvertisement,
    fragment: &PlanFragment,
) -> Result<(), String> {
    if fragment.host_id != advertisement.host_id {
        return Err("fragment is assigned to a different host".to_string());
    }
    for placement in &fragment.placements {
        if placement.host_id != advertisement.host_id
            || placement.boot_id != advertisement.boot_id
            || placement.offer_generation != advertisement.offer_generation
        {
            return Err(format!(
                "placement '{}' does not target the current host boot and offer",
                placement.placement_id.as_str()
            ));
        }
        let capability = advertisement
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == placement.capability_id)
            .ok_or_else(|| {
                format!(
                    "placement '{}' names an unavailable capability",
                    placement.placement_id.as_str()
                )
            })?;
        if let Some(binding) = placement.resources.iter().find(|binding| {
            !advertisement
                .resources
                .iter()
                .any(|offer| offer.pool_id == binding.pool_id)
        }) {
            return Err(format!(
                "resource pool '{}' is not offered by the current host",
                binding.pool_id.as_str()
            ));
        }
        let resources_match = capability.resource_requirements.len() == placement.resources.len()
            && capability.resource_requirements.iter().all(|requirement| {
                placement.resources.iter().any(|binding| {
                    advertisement
                        .resources
                        .iter()
                        .find(|offer| offer.pool_id == binding.pool_id)
                        .is_some_and(|offer| {
                            resource_binding_satisfies(binding, requirement, offer)
                        })
                })
            });
        let authority_match = capability.authority_requirements.len() == placement.authority.len()
            && capability.authority_requirements.iter().all(|requirement| {
                placement.authority.iter().any(|binding| {
                    !binding.grant_id.as_str().is_empty()
                        && binding.contract_id == requirement.contract_id
                        && binding.host_call_contract_id == requirement.host_call_contract_id
                        && binding.subject_kind == requirement.subject_kind
                        && binding.host_id == placement.host_id
                        && binding.boot_id == placement.boot_id
                        && binding.capability_id == placement.capability_id
                })
            });
        if capability.kind_id != placement.kind_id
            || capability.kind_contract_revision != placement.kind_contract_revision
            || capability.semantic_contract != placement.semantic_contract
            || capability.realization_properties != placement.realization_properties
            || capability.implementation.execution_profile_id != placement.execution_profile_id
            || capability.implementation.implementation_id != placement.implementation_id
            || capability.implementation.artifact_id != placement.artifact_id
            || capability.inputs != placement.inputs
            || capability.outputs != placement.outputs
            || capability.host_calls != placement.host_calls
            || !resources_match
            || !authority_match
        {
            return Err(format!(
                "placement '{}' differs from the installed exact capability",
                placement.placement_id.as_str()
            ));
        }
        let active_instances = fragment
            .placements
            .iter()
            .filter(|candidate| candidate.capability_id == placement.capability_id)
            .count();
        if active_instances > usize::from(capability.limits.max_active_instances) {
            return Err(format!(
                "capability '{}' active-instance limit exceeded",
                capability.capability_id.as_str()
            ));
        }
    }
    Ok(())
}
