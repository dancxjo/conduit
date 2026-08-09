use crate::policy::select_realization_matching;
use crate::{HardRealizationRequirements, PlacementChoice, PlannerError, RealizationPolicy};
use conduit_core::{
    CapabilityOffer, HostAdvertisement, ResourceHealth, ResourceObservation, ResourcePoolId,
};
use conduit_form::CheckedGear;
use std::collections::{BTreeMap, BTreeSet};

pub fn select_realization_with_observations(
    gear: &CheckedGear,
    hosts: &[HostAdvertisement],
    requirements: &HardRealizationRequirements,
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> Result<PlacementChoice, PlannerError> {
    validate_resource_observations(hosts, observations)?;
    select_realization_matching(
        gear,
        hosts,
        requirements,
        policy,
        |host, offer| observations_admit(host, offer, observations),
        Some(PlannerError::CurrentResourceObservationUnavailable(
            format!(
                "gear '{}' has no realization with current observed resources",
                gear.gear_id.as_str()
            ),
        )),
    )
}

pub(crate) fn validate_resource_observations(
    hosts: &[HostAdvertisement],
    observations: &[ResourceObservation],
) -> Result<(), PlannerError> {
    let mut scopes = BTreeSet::new();
    for observation in observations {
        if observation.host_id.as_str().is_empty()
            || observation.boot_id.as_str().is_empty()
            || observation.pool_id.as_str().is_empty()
            || observation.class_id.as_str().is_empty()
            || observation.clue_id.as_str().is_empty()
        {
            return invalid("observation identities must be non-empty");
        }
        let Some(host) = hosts
            .iter()
            .find(|host| host.host_id == observation.host_id)
        else {
            return invalid("observation host is absent from the planning hosts");
        };
        if host.boot_id != observation.boot_id
            || host.offer_generation != observation.offer_generation
        {
            return invalid("observation boot or offer generation is stale");
        }
        let Some(pool) = host.resources.iter().find(|pool| {
            pool.pool_id == observation.pool_id && pool.class_id == observation.class_id
        }) else {
            return invalid("observation does not name an advertised resource pool");
        };
        if observation.unreserved_units > pool.capacity_units
            || observation.utilized_units > pool.capacity_units
            || (observation.health == ResourceHealth::Unavailable
                && observation.unreserved_units != 0)
        {
            return invalid("observation units or health exceed the stable pool offer");
        }
        if !scopes.insert((
            observation.host_id.clone(),
            observation.boot_id.clone(),
            observation.offer_generation,
            observation.pool_id.clone(),
        )) {
            return invalid("resource observation scope must be unique");
        }
    }
    Ok(())
}

pub(crate) fn observations_admit(
    host: &HostAdvertisement,
    offer: &CapabilityOffer,
    observations: &[ResourceObservation],
) -> bool {
    let mut remaining = observations
        .iter()
        .filter(|observation| {
            observation.host_id == host.host_id
                && observation.boot_id == host.boot_id
                && observation.offer_generation == host.offer_generation
                && observation.health == ResourceHealth::Ready
        })
        .map(|observation| (observation.pool_id.clone(), observation.unreserved_units))
        .collect::<BTreeMap<ResourcePoolId, u32>>();

    for requirement in &offer.resource_requirements {
        let Some(pool) = host.resources.iter().find(|pool| {
            pool.class_id == requirement.class_id
                && remaining.get(&pool.pool_id).is_some_and(|available| {
                    if requirement.compute.is_some() {
                        conduit_core::compute_reservation(requirement, pool, *available).is_some()
                    } else {
                        *available >= requirement.units
                    }
                })
        }) else {
            return false;
        };
        let available = remaining
            .get_mut(&pool.pool_id)
            .expect("selected observed pool has remaining capacity");
        // Observation admission reserves the requirement minimum. Preferred
        // lanes are distributed only after the whole form's minima are known.
        let selected = requirement.units;
        *available -= selected;
    }
    true
}

fn invalid<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidResourceObservation(detail.to_string()))
}
