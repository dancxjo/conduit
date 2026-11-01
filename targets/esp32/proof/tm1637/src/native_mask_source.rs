//! Exact checked Source graph/configuration, independent of received Plan IDs.
pub fn matches(
    planned: &conduit_face::PlannedMaskPlot,
    configurations: &[(&str, &str, &str)],
    connections: &[(&str, &str, &str, &str)],
) -> bool {
    let [fragment] = planned.plan.fragments.as_slice() else {
        return false;
    };
    if fragment.placements.len() != configurations.len()
        || fragment.connections.len() != connections.len()
        || fragment.fore_ports.len() != 3
    {
        return false;
    }
    for (gear, kind, label) in configurations {
        if fragment
            .placements
            .iter()
            .filter(|p| {
                p.gear_id.as_str() == *gear
                    && p.kind_id.as_str() == *kind
                    && conduit_core::face_data_label(&p.configuration).ok() == Some(*label)
            })
            .count()
            != 1
        {
            return false;
        }
    }
    for (source, source_port, sink, sink_port) in connections {
        let placement = |gear: &str| {
            fragment
                .placements
                .iter()
                .find(|p| p.gear_id.as_str() == gear)
                .map(|p| &p.placement_id)
        };
        if fragment
            .connections
            .iter()
            .filter(|cord| {
                Some(&cord.source_placement_id) == placement(source)
                    && Some(&cord.sink_placement_id) == placement(sink)
                    && cord.source_port_id.as_str() == *source_port
                    && cord.sink_port_id.as_str() == *sink_port
                    && cord.item_capacity == 1
                    && cord.byte_capacity == 8192
            })
            .count()
            != 1
        {
            return false;
        }
    }
    true
}
