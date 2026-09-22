//! Default queue budgets are preferences bounded by both selected offers.
//! Explicit caller requirements still go through the strict admission path.

use super::*;
use conduit_core::{DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY};

pub fn plan_expanded_canonical(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
) -> Result<Plan, PlannerError> {
    form.validate_expansion()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    let mut limits = BTreeMap::new();
    for connection in &form.connections {
        let capability = |gear: &conduit_core::GearId| {
            let choice = placements
                .by_gear
                .get(gear)
                .ok_or_else(|| PlannerError::MissingPlacement(gear.as_str().to_string()))?;
            crate::find_capability(hosts, &choice.host_id, &choice.capability_id)
        };
        let source = capability(&connection.source_gear_id)?;
        let sink = capability(&connection.sink_gear_id)?;
        if [source, sink]
            .iter()
            .any(|offer| offer.limits.max_queue_items == 0 || offer.limits.max_queue_bytes == 0)
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires nonzero queue capacity",
                connection.source_gear_id.as_str(),
                connection.sink_gear_id.as_str(),
            )));
        }
        limits.insert(
            (
                connection.source_gear_id.clone(),
                connection.source_port_id.clone(),
                connection.sink_gear_id.clone(),
                connection.sink_port_id.clone(),
            ),
            ConnectionQueueLimits {
                item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY
                    .min(source.limits.max_queue_items)
                    .min(sink.limits.max_queue_items),
                byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY
                    .min(source.limits.max_queue_bytes)
                    .min(sink.limits.max_queue_bytes),
            },
        );
    }
    plan_expanded_canonical_with_connection_limits(
        form,
        hosts,
        placements,
        bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
}
