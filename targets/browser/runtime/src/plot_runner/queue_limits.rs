//! Finite connection queue admission for installed browser value profiles.
use std::collections::BTreeMap;

pub(crate) fn finite_connection_limits(
    plot: &conduit_plot::ExpandedCanonicalPlot,
    default_bytes: u32,
) -> BTreeMap<conduit_planner::ConnectionEndpoints, conduit_planner::ConnectionQueueLimits> {
    plot.connections
        .iter()
        .filter_map(|connection| {
            let source = plot
                .gears
                .iter()
                .find(|gear| gear.gear_id == connection.source_gear_id)?;
            let output = source
                .outputs
                .iter()
                .find(|port| port.port_id == connection.source_port_id)?;
            let sink = plot
                .gears
                .iter()
                .find(|gear| gear.gear_id == connection.sink_gear_id)?;
            let sample_bound = [
                (
                    source,
                    conduit_core::FrontValueLocation::Output(connection.source_port_id.clone()),
                ),
                (
                    sink,
                    conduit_core::FrontValueLocation::Input(connection.sink_port_id.clone()),
                ),
            ]
            .into_iter()
            .find_map(|(gear, location)| {
                if !matches!(
                    gear.kind_id.as_str(),
                    conduit_semantic_catalog::TIME_SAMPLE_KIND
                        | conduit_semantic_catalog::DISTANCE_FREQUENCY_MAP_KIND
                ) {
                    return None;
                }
                if location
                    == conduit_core::FrontValueLocation::Input(conduit_core::port_id("cadence"))
                {
                    return Some(conduit_time::TICK_ENCODED_LEN);
                }
                gear.semantic_contract
                    .value_contracts()
                    .iter()
                    .find(|entry| entry.location == location)
                    .map(|entry| entry.contract.maximum_bytes)
            });
            let dimension_bound = (output.value_kind.as_str() == conduit_core::DIMENSION_INFO_ID)
                .then_some(conduit_core::DIMENSION_ENCODED_LEN as u32);
            let application_view_bound = (output.value_kind.as_str()
                == conduit_face::APPLICATION_VIEW_INFO_ID)
                .then_some(conduit_face::MAX_APPLICATION_VIEW_BYTES as u32);
            let bound = dimension_bound
                .or(application_view_bound)
                .or(sample_bound)
                .or_else(|| {
                    crate::installed_browser::measurement_limits::value_queue_bound(
                        &output.value_kind,
                    )
                    .map(|bound| bound.max(default_bytes))
                })?;
            Some((
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bound.max(1),
                },
            ))
        })
        .collect()
}
