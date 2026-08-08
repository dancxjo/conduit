use crate::{PlacementChoices, PlannerError};
use conduit_core::{CapabilityId, HostAdvertisement, HostId, ResourcePoolId};
use conduit_form::CheckedForm;
use std::collections::BTreeMap;

pub(crate) type RemainingComputeMinimum = BTreeMap<(HostId, ResourcePoolId), u32>;

/// Admits every scalable minimum before Plan construction distributes spare
/// lanes toward preferences. This is the compute-specific part of the existing
/// whole-form finite resource allocator, not a separate scheduler.
pub(crate) fn admit_minima(
    form: &CheckedForm,
    realm: &BTreeMap<HostId, &HostAdvertisement>,
    placements: &PlacementChoices,
) -> Result<RemainingComputeMinimum, PlannerError> {
    let mut remaining = BTreeMap::<(HostId, ResourcePoolId), u32>::new();
    for operation in &form.operations {
        let choice = placements
            .by_operation
            .get(&operation.operation_id)
            .ok_or_else(|| {
                PlannerError::MissingPlacement(operation.operation_id.as_str().to_string())
            })?;
        let host = realm
            .get(&choice.host_id)
            .ok_or_else(|| PlannerError::UnknownHost(choice.host_id.as_str().to_string()))?;
        let capability = host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .ok_or_else(|| unknown_capability(&choice.capability_id))?;
        for requirement in &capability.resource_requirements {
            if requirement.compute.is_none() {
                continue;
            }
            let mut pools = host
                .resources
                .iter()
                .filter(|pool| pool.class_id == requirement.class_id);
            let pool = pools.next().ok_or_else(|| {
                PlannerError::UnavailableResource(requirement.class_id.as_str().to_string())
            })?;
            if pools.next().is_some() {
                return Err(PlannerError::InvalidResourceContract(format!(
                    "host '{}' has multiple pools for class '{}' in the first planning profile",
                    host.host_id.as_str(),
                    requirement.class_id.as_str()
                )));
            }
            let minimum = remaining
                .entry((host.host_id.clone(), pool.pool_id.clone()))
                .or_insert(0);
            *minimum = minimum.checked_add(requirement.units).ok_or_else(|| {
                PlannerError::ResourceCapacityExceeded(pool.pool_id.as_str().to_string())
            })?;
            if *minimum > pool.capacity_units {
                return Err(PlannerError::ResourceCapacityExceeded(format!(
                    "pool '{}' compute minima require {} lanes above capacity {}",
                    pool.pool_id.as_str(),
                    *minimum,
                    pool.capacity_units
                )));
            }
        }
    }
    Ok(remaining)
}

fn unknown_capability(capability_id: &CapabilityId) -> PlannerError {
    PlannerError::UnknownCapability(capability_id.as_str().to_string())
}
