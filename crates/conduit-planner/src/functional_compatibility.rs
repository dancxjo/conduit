use crate::prelude::*;
use crate::{PlacementChoice, PlacementChoices, PlannerError};
use alloc::collections::BTreeMap;
use conduit_core::HostAdvertisement;
use conduit_form::CheckedGear;

pub(crate) fn default_placements_unvalidated(
    gears: &[CheckedGear],
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    if hosts.is_empty() {
        return Err(PlannerError::UnknownHost("hosts is empty".to_string()));
    }
    let mut by_gear = BTreeMap::new();
    let mut selected_counts = BTreeMap::new();
    for gear in gears {
        let mut candidates = hosts
            .iter()
            .enumerate()
            .flat_map(|(host_index, host)| {
                host.capabilities
                    .iter()
                    .map(move |offer| (host_index, host, offer))
            })
            .filter(|(_, _, offer)| offer.checked_face() == gear.checked_face())
            .filter(|(_, host, offer)| {
                selected_counts
                    .get(&(host.host_id.clone(), offer.capability_id.clone()))
                    .copied()
                    .unwrap_or(0)
                    < offer.limits.max_active_instances
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(host_index, _, offer)| {
            (
                *host_index,
                offer.kind_id != gear.kind_id,
                offer.kind_contract_revision != gear.kind_contract_revision,
                offer.capability_id.clone(),
            )
        });
        let (_, host, offer) = candidates
            .first()
            .copied()
            .ok_or_else(|| PlannerError::UnknownCapability(gear.kind_id.as_str().to_string()))?;
        *selected_counts
            .entry((host.host_id.clone(), offer.capability_id.clone()))
            .or_default() += 1;
        by_gear.insert(
            gear.gear_id.clone(),
            PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: offer.capability_id.clone(),
            },
        );
    }
    Ok(PlacementChoices { by_gear })
}
