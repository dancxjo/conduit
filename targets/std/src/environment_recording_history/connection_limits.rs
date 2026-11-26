//! Pure queue admission for the exact captured-history Source's typed Cords.
use conduit_planner::{ConnectionEndpoints, ConnectionQueueLimits};
use conduit_plot::ExpandedCanonicalPlot;
use std::collections::BTreeMap;

pub fn environment_recording_history_connection_limits(
    plot: &ExpandedCanonicalPlot,
) -> Result<BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>, String> {
    let source = |id| {
        plot.gears.iter().any(|gear| {
            &gear.gear_id == id
                && gear.kind_id.as_str()
                    == conduit_semantic_catalog::ENVIRONMENT_RECORDING_HISTORY_KIND
        })
    };
    let mut limits = BTreeMap::new();
    for connection in &plot.connections {
        let bytes = if connection.source_port_id.as_str() == "observations"
            && source(&connection.source_gear_id)
        {
            conduit_dht11_observation::reported_generation::maximum_canonical_bytes()
                .map_err(|why| format!("captured history queue profile: {why:?}"))?
        } else if connection.sink_port_id.as_str() == "tick" && source(&connection.sink_gear_id) {
            conduit_time::TICK_ENCODED_LEN
        } else {
            continue;
        };
        limits.insert(
            (
                connection.source_gear_id.clone(),
                connection.source_port_id.clone(),
                connection.sink_gear_id.clone(),
                connection.sink_port_id.clone(),
            ),
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: bytes,
            },
        );
    }
    Ok(limits)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_recording_source_uses_the_same_exact_typed_bounds() {
        use conduit_plot::*;
        let mut startup = StartupCatalog::new();
        let mut catalog = ProfileCatalog::new();
        conduit_time::install_time_every_catalog(&mut startup, &mut catalog).unwrap();
        conduit_semantic_catalog::install_environment_recording_history_kind(
            &mut startup,
            &mut catalog,
        )
        .unwrap();
        conduit_semantic_catalog::install_reported_generation_face_kind(&mut startup, &mut catalog)
            .unwrap();
        let checked = check_syntax_document(
            &parse_syntax_document(include_str!(
                "../../../../plots/environment-recording-history/main.conduit"
            )),
            &startup,
        )
        .unwrap();
        let plot = expand_canonical_plot_for_authoring(
            &checked,
            "environment-recording-history",
            &catalog,
        )
        .unwrap()
        .expanded;
        let limits = environment_recording_history_connection_limits(&plot).unwrap();
        assert_eq!(limits.len(), 2);
        let mut capacities = limits
            .values()
            .map(|limit| {
                assert_eq!(limit.item_capacity, 1);
                limit.byte_capacity
            })
            .collect::<Vec<_>>();
        let mut wrong_kind = plot.clone();
        wrong_kind
            .gears
            .iter_mut()
            .find(|gear| {
                gear.kind_id.as_str()
                    == conduit_semantic_catalog::ENVIRONMENT_RECORDING_HISTORY_KIND
            })
            .unwrap()
            .kind_id = "example/not-history".into();
        assert!(environment_recording_history_connection_limits(&wrong_kind)
            .unwrap()
            .is_empty());
        let mut wrong_ports = plot;
        for cord in &mut wrong_ports.connections {
            if cord.sink_port_id.as_str() == "tick" {
                cord.sink_port_id = "other-tick".into();
            }
            if cord.source_port_id.as_str() == "observations" {
                cord.source_port_id = "other-observations".into();
            }
        }
        assert!(
            environment_recording_history_connection_limits(&wrong_ports)
                .unwrap()
                .is_empty()
        );
        capacities.sort_unstable();
        assert_eq!(
            capacities,
            [
                conduit_time::TICK_ENCODED_LEN,
                conduit_dht11_observation::reported_generation::maximum_canonical_bytes().unwrap()
            ]
        );
    }
}
