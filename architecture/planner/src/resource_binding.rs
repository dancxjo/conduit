use crate::{protected_resources::bind_protected_resource, PlannerError};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    format,
    string::ToString,
    vec::Vec,
};
use conduit_core::{
    CapabilityOffer, HostAdvertisement, HostId, ProtectedResourceGrant, ResourceBinding,
    ResourceHandleId, ResourcePoolId,
};
use conduit_form::CheckedGear;

pub(crate) struct ResourcePlanningState<'a> {
    pub writers: &'a mut BTreeSet<(HostId, ResourcePoolId)>,
    pub usage: &'a mut BTreeMap<(HostId, ResourcePoolId), u32>,
    pub compute_minimum: &'a mut BTreeMap<(HostId, ResourcePoolId), u32>,
    pub protected_handles: &'a mut BTreeSet<ResourceHandleId>,
}

pub(crate) fn bind_resources(
    host: &HostAdvertisement,
    capability: &CapabilityOffer,
    gear: &CheckedGear,
    protected_resource_grants: &[ProtectedResourceGrant],
    state: ResourcePlanningState<'_>,
) -> Result<Vec<ResourceBinding>, PlannerError> {
    let resource_usage = state.usage;
    let remaining_compute_minimum = state.compute_minimum;
    let consumed_protected_handles = state.protected_handles;
    let mut resource_bindings = Vec::with_capacity(capability.resource_requirements.len());
    for requirement in &capability.resource_requirements {
        let mut matches = host
            .resources
            .iter()
            .filter(|resource| resource.class_id == requirement.class_id);
        let Some(resource) = matches.next() else {
            return Err(PlannerError::UnavailableResource(format!(
                "host '{}' has no pool for class '{}'",
                host.host_id.as_str(),
                requirement.class_id.as_str()
            )));
        };
        if matches.next().is_some() {
            return Err(PlannerError::InvalidResourceContract(format!(
                "host '{}' has multiple pools for class '{}' in the first planning profile",
                host.host_id.as_str(),
                requirement.class_id.as_str()
            )));
        }
        let used = resource_usage
            .entry((host.host_id.clone(), resource.pool_id.clone()))
            .or_insert(0);
        let key = (host.host_id.clone(), resource.pool_id.clone());
        let reserved_for_later = if requirement.compute.is_some() {
            let remaining = remaining_compute_minimum
                .get_mut(&key)
                .expect("compute minimum was pre-admitted");
            *remaining -= requirement.units;
            *remaining
        } else {
            0
        };
        let available = resource
            .capacity_units
            .saturating_sub(*used)
            .saturating_sub(reserved_for_later);
        let compute = match &requirement.compute {
                Some(_) => Some(
                    conduit_core::compute_reservation(requirement, resource, available)
                        .ok_or_else(|| {
                            PlannerError::UnavailableResource(format!(
                                "pool '{}' cannot satisfy the compute range, service, or topology contract",
                                resource.pool_id.as_str()
                            ))
                        })?,
                ),
                None => None,
            };
        let selected_units = compute
            .as_ref()
            .map_or(requirement.units, |reservation| reservation.selected_lanes);
        *used = used.checked_add(selected_units).ok_or_else(|| {
            PlannerError::ResourceCapacityExceeded(resource.pool_id.as_str().to_string())
        })?;
        if *used > resource.capacity_units {
            return Err(PlannerError::ResourceCapacityExceeded(format!(
                "pool '{}' requires {} units above capacity {}",
                resource.pool_id.as_str(),
                *used,
                resource.capacity_units
            )));
        }
        if requirement.content.as_ref().is_some_and(|content| {
            content.access == conduit_core::ResourceAccessMode::WriteCandidatePublish
        }) && !state
            .writers
            .insert((host.host_id.clone(), resource.pool_id.clone()))
        {
            return Err(PlannerError::ResourceContentRefused(
                conduit_core::ResourceContentRefusal::MultipleWriters,
            ));
        }
        let protected = bind_protected_resource(
            requirement,
            protected_resource_grants,
            gear,
            host,
            capability,
            consumed_protected_handles,
        )?;
        resource_bindings.push(ResourceBinding {
            content: conduit_core::bind_resource_content(
                requirement,
                resource,
                &host.host_id,
                &host.boot_id,
            )
            .map_err(PlannerError::ResourceContentRefused)?,
            pool_id: resource.pool_id.clone(),
            class_id: resource.class_id.clone(),
            units: selected_units,
            protected,
            compute,
        });
    }
    resource_bindings.sort();

    Ok(resource_bindings)
}
