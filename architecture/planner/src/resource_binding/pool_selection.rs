//! Resolve finite resource pools from selected capability provenance, not class aliases.
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
    let invalid = || {
        PlannerError::InvalidResourceContract(format!(
            "capability '{}' has invalid or ambiguous Base resource provenance on host '{}'",
            capability.capability_id.as_str(),
            host.host_id.as_str(),
        ))
    };
    let mut owners = host
        .bases
        .iter()
        .filter(|base| base.capability_ids.contains(&capability.capability_id));
    let owner = owners.next();
    if owners.next().is_some() {
        return Err(invalid());
    }
    if let Some(owner) = owner {
        if owner.lifecycle != BaseLifecycle::Ready || owner.provider_generation == 0 {
            return Err(invalid());
        }
        for (index, pool) in owner.resource_pool_ids.iter().enumerate() {
            if owner.resource_pool_ids[..index].contains(pool)
                || !host
                    .resources
                    .iter()
                    .any(|resource| &resource.pool_id == pool)
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
            .any(|pool| pool.class_id == *class && base.resource_pool_ids.contains(&pool.pool_id))
    });
    let mut pools = host.resources.iter().filter(|pool| {
        pool.class_id == *class
            && match owner {
                None => true,
                Some(base) if owned_pool => base.resource_pool_ids.contains(&pool.pool_id),
                Some(_) => !host
                    .bases
                    .iter()
                    .any(|base| base.resource_pool_ids.contains(&pool.pool_id)),
            }
    });
    let pool = pools.next().ok_or_else(|| {
        PlannerError::UnavailableResource(format!(
            "host '{}' has no selected pool for capability '{}' class '{}'",
            host.host_id.as_str(),
            capability.capability_id.as_str(),
            class.as_str(),
        ))
    })?;
    if pools.next().is_some() {
        return Err(invalid());
    }
    if owned_pool
        && host
            .bases
            .iter()
            .filter(|base| base.resource_pool_ids.contains(&pool.pool_id))
            .count()
            != 1
    {
        return Err(invalid());
    }
    Ok(pool)
}

#[cfg(test)]
mod tests;
