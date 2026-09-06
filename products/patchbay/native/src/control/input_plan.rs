//! Exact native-input Cord budgets bounded by both selected offers.
use conduit_core::{HostAdvertisement, Plan};
use conduit_form::ExpandedCanonicalForm;
use conduit_planner::{ConnectionQueueLimits, PlanningOptions};
use std::collections::BTreeMap;

pub(super) fn plan(
    form: &ExpandedCanonicalForm,
    advertisement: &HostAdvertisement,
) -> Result<Plan, String> {
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(form, &hosts)
        .map_err(|error| error.to_string())?;
    let mut limits = BTreeMap::new();
    for connection in &form.connections {
        let capability = |gear| {
            let choice = placements
                .by_gear
                .get(gear)
                .ok_or("native input placement missing")?;
            advertisement
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == choice.capability_id)
                .ok_or("native input capability missing")
        };
        let source = capability(&connection.source_gear_id)?;
        let sink = capability(&connection.sink_gear_id)?;
        limits.insert(
            (
                connection.source_gear_id.clone(),
                connection.source_port_id.clone(),
                connection.sink_gear_id.clone(),
                connection.sink_port_id.clone(),
            ),
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: source
                    .limits
                    .max_queue_bytes
                    .min(sink.limits.max_queue_bytes),
            },
        );
    }
    conduit_planner::plan_expanded_canonical_with_connection_limits(
        form,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
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
    .map_err(|error| error.to_string())
}
