//! Conservative prefix rejection; ordinary Plan construction owns feasibility.
use super::*;

/// Minimum demand is a lower bound even when compute reservations expand to
/// additional lanes. Never use a prefix's selected elastic allocation as the
/// reservation of a complete assignment. Ambiguous/missing facts defer to the
/// ordinary admission path rather than manufacturing a pruning certificate.
pub(super) fn exceeds_minimum_capacity(
    inputs: CostPlanningInputs<'_, '_>,
    grids: &[Vec<Candidate<'_>>],
    chosen: &[usize],
) -> bool {
    let mut totals = BTreeMap::new();
    for (depth, index) in chosen.iter().enumerate() {
        let choice = &grids[depth][*index].choice;
        let Some(host) = inputs
            .hosts
            .iter()
            .find(|host| host.host_id == choice.host_id)
        else {
            return false;
        };
        let Some(capability) = host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
        else {
            return false;
        };
        for requirement in &capability.resource_requirements {
            let mut pools = host
                .resources
                .iter()
                .filter(|pool| pool.class_id == requirement.class_id);
            let Some(pool) = pools.next() else {
                return false;
            };
            if pools.next().is_some() {
                return false;
            }
            let key = (
                &host.host_id,
                &host.boot_id,
                host.offer_generation,
                &pool.pool_id,
                &pool.class_id,
            );
            let used = totals.entry(key).or_insert(0u32);
            let Some(sum) = used.checked_add(requirement.units) else {
                return true;
            };
            *used = sum;
            if sum > pool.capacity_units {
                return true;
            }
            let mut observations = inputs
                .observations
                .iter()
                .map(TimedResourceObservation::observation)
                .filter(|value| {
                    value.host_id == host.host_id
                        && value.boot_id == host.boot_id
                        && value.offer_generation == host.offer_generation
                        && value.pool_id == pool.pool_id
                        && value.class_id == pool.class_id
                });
            if let Some(observation) = observations.next() {
                if observations.next().is_none() && sum > observation.unreserved_units {
                    return true;
                }
            }
        }
    }
    false
}
