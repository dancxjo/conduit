use crate::planning_input::PlanningGears;
use crate::prelude::*;
use crate::{PlacementChoice, PlacementChoices, PlannerError};
use alloc::collections::BTreeMap;
use conduit_core::HostAdvertisement;
use conduit_plot::{CheckedConnection, CheckedGear};
mod quantity_selection;

pub(crate) fn default_placements_unvalidated(
    gears: &[CheckedGear],
    connections: &[CheckedConnection],
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    default_planning_placements(&PlanningGears::borrowed(gears), connections, hosts)
}

pub(crate) fn default_planning_placements(
    gears: &PlanningGears<'_>,
    connections: &[CheckedConnection],
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    if quantity_selection::needed(gears, hosts)? {
        return quantity_selection::select(gears, connections, hosts);
    }
    if hosts.is_empty() {
        return Err(PlannerError::UnknownHost("hosts is empty".to_string()));
    }
    let mut by_gear = BTreeMap::new();
    let mut selected_counts = BTreeMap::new();
    for gear in gears.iter() {
        let gear = gear?;
        let candidates = hosts
            .iter()
            .enumerate()
            .flat_map(|(host_index, host)| {
                host.capabilities
                    .iter()
                    .map(move |offer| (host_index, host, offer))
            })
            .filter(|(_, _, offer)| gear.accepts_realization(offer))
            .collect::<Vec<_>>();
        let mut candidates =
            crate::language_coverage::filter_candidates(&gear, candidates, |candidate| {
                (candidate.1, candidate.2)
            })?;
        candidates.retain(|(_, host, offer)| {
            selected_counts
                .get(&(host.host_id.clone(), offer.capability_id.clone()))
                .copied()
                .unwrap_or(0)
                < offer.limits.max_active_instances
        });
        candidates.sort_by(|left, right| {
            compare_default_candidates_ref(
                gear.as_ref().into(),
                left.0,
                left.2.into(),
                right.0,
                right.2.into(),
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

pub(crate) fn compare_default_candidates_ref(
    gear: conduit_plot::CheckedGearRef<'_>,
    left_host: usize,
    left: conduit_core::CapabilityOfferRef<'_>,
    right_host: usize,
    right: conduit_core::CapabilityOfferRef<'_>,
) -> core::cmp::Ordering {
    (left_host, left.kind_id != gear.kind_id, left.capability_id).cmp(&(
        right_host,
        right.kind_id != gear.kind_id,
        right.capability_id,
    ))
}
