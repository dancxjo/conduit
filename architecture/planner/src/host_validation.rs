//! Complete resource/Base validation shared by ordinary and indexed offers.
use crate::PlannerError;
pub(crate) fn validate_host_resources_ref(
    host: conduit_core::HostAdvertisementRef<'_>,
) -> Result<(), PlannerError> {
    validate_host_resources_with_capabilities(host, |id| {
        Ok(host
            .capabilities
            .iter()
            .any(|offer| offer.capability_id == id))
    })
}
pub(crate) fn validate_host_resources_with_capabilities(
    host: conduit_core::HostAdvertisementRef<'_>,
    mut contains: impl FnMut(&str) -> Result<bool, PlannerError>,
) -> Result<(), PlannerError> {
    if host.resources.iter().any(|resource| {
        resource.pool_id.is_empty()
            || resource.class_id.is_empty()
            || resource.capacity_units == 0
            || resource
                .compute
                .is_some_and(|compute| !compute.is_valid_for_capacity(resource.capacity_units))
    }) || host
        .resources
        .iter()
        .zip(host.resources.iter().skip(1))
        .any(|(left, right)| left.pool_id >= right.pool_id)
    {
        return Err(PlannerError::InvalidResourceContract(format!(
            "host '{}' pools must have non-empty identities, positive capacity, and unique pool-id ordering",
            host.host_id
        )));
    }
    for base in host.bases.iter() {
        if base.lifecycle != conduit_core::BaseLifecycle::Ready
            || base.base_id.is_empty()
            || base.provider_instance_id.is_empty()
            || base.provider_generation == 0
            || base.implementation_id.is_empty()
            || base.mechanism_family.is_empty()
            || base.capability_ids.is_empty() && base.resource_pool_ids.is_empty()
            || base
                .resource_pool_ids
                .iter()
                .any(|id| !host.resources.iter().any(|offer| offer.pool_id == id))
        {
            return Err(PlannerError::InvalidResourceContract(format!(
                "host '{}' has invalid current Base provenance",
                host.host_id
            )));
        }
        for id in base.capability_ids.iter() {
            if !contains(id)? {
                return Err(PlannerError::InvalidResourceContract(format!(
                    "host '{}' has invalid current Base provenance",
                    host.host_id
                )));
            }
        }
    }
    Ok(())
}
