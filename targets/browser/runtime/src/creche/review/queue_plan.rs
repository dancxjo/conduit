//! Review uses ordinary finite Cord budgets supported by both selected offers.
use conduit_core::{BaseImplementationId, HostAdvertisement};
use conduit_form::ExpandedCanonicalForm;
use conduit_planner::{ConnectionQueueLimits, PlanningOptions};
use std::collections::BTreeMap;

pub(super) fn review(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &conduit_planner::PlacementChoices,
    bases: &[BaseImplementationId],
) -> Result<(), String> {
    let mut limits = BTreeMap::new();
    for cord in &form.connections {
        let capability = |gear| {
            let choice = placements
                .by_gear
                .get(gear)
                .ok_or("missing review placement")?;
            hosts
                .iter()
                .find(|host| host.host_id == choice.host_id)
                .and_then(|host| {
                    host.capabilities
                        .iter()
                        .find(|offer| offer.capability_id == choice.capability_id)
                })
                .ok_or("missing selected review capability")
        };
        let source = capability(&cord.source_gear_id)?;
        let sink = capability(&cord.sink_gear_id)?;
        limits.insert(
            (
                cord.source_gear_id.clone(),
                cord.source_port_id.clone(),
                cord.sink_gear_id.clone(),
                cord.sink_port_id.clone(),
            ),
            ConnectionQueueLimits {
                item_capacity: source
                    .limits
                    .max_queue_items
                    .min(sink.limits.max_queue_items)
                    .min(4),
                byte_capacity: source
                    .limits
                    .max_queue_bytes
                    .min(sink.limits.max_queue_bytes),
            },
        );
    }
    conduit_planner::plan_expanded_canonical_with_connection_limits(
        form,
        hosts,
        placements,
        bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}
