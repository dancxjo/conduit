//! Resolve finite resource pools from actual selected capability provenance.
use crate::PlannerError;
use alloc::format;
use conduit_core::{
    BaseLifecycle, CapabilityOffer, HostAdvertisement, ResourceClassId, ResourceOffer,
};
pub(crate) fn resolve_resource_pool<'a>(
    host: &'a HostAdvertisement,
    capability: &CapabilityOffer,
    class: &ResourceClassId,
) -> Result<&'a ResourceOffer, PlannerError> {
    let index = resolve_resource_pool_index(host.into(), capability.into(), class.as_str())?;
    Ok(&host.resources[index])
}
pub(crate) fn resolve_resource_pool_index(
    host: conduit_core::HostAdvertisementRef<'_>,
    capability: conduit_core::CapabilityOfferRef<'_>,
    class: &str,
) -> Result<usize, PlannerError> {
    let invalid = || {
        PlannerError::InvalidResourceContract(format!(
            "capability '{}' has invalid or ambiguous Base resource provenance on host '{}'",
            capability.capability_id, host.host_id,
        ))
    };
    let mut owners = host
        .bases
        .iter()
        .filter(|base| base.capability_ids.contains(capability.capability_id));
    let owner = owners.next();
    if owners.next().is_some() {
        return Err(invalid());
    }
    if let Some(owner) = owner {
        if owner.lifecycle != BaseLifecycle::Ready || owner.provider_generation == 0 {
            return Err(invalid());
        }
        for (index, pool) in owner.resource_pool_ids.iter().enumerate() {
            if owner
                .resource_pool_ids
                .iter()
                .take(index)
                .any(|previous| previous == pool)
                || !host
                    .resources
                    .iter()
                    .any(|resource| resource.pool_id == pool)
            {
                return Err(invalid());
            }
        }
    }
    // Own matching pools take precedence even when their capacity is too small.
    // A capability Base may also consume Host-shared resources: only pools not
    // advertised as owned by ANY Base qualify when no matching owned pool exists.
    // Base-free offers retain the existing unique-class profile unchanged.
    let owned_pool = owner.is_some_and(|base| {
        host.resources
            .iter()
            .any(|pool| pool.class_id == class && base.resource_pool_ids.contains(pool.pool_id))
    });
    let mut pools = host.resources.iter().enumerate().filter(|(_, pool)| {
        pool.class_id == class
            && match owner {
                None => true,
                Some(base) if owned_pool => base.resource_pool_ids.contains(pool.pool_id),
                Some(_) => !host
                    .bases
                    .iter()
                    .any(|base| base.resource_pool_ids.contains(pool.pool_id)),
            }
    });
    let (index, pool) = pools.next().ok_or_else(|| {
        PlannerError::UnavailableResource(format!(
            "host '{}' has no selected pool for capability '{}' class '{}'",
            host.host_id, capability.capability_id, class,
        ))
    })?;
    if pools.next().is_some() {
        return Err(invalid());
    }
    if owned_pool
        && host
            .bases
            .iter()
            .filter(|base| base.resource_pool_ids.contains(pool.pool_id))
            .count()
            != 1
    {
        return Err(invalid());
    }
    Ok(index)
}

#[cfg(test)]
mod tests;
