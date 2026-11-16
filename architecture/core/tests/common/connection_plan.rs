use conduit_core::*;
#[path = "sealed_state.rs"]
mod state;

pub fn line() -> AdmittedLine {
    AdmittedLine {
        line_id: "line/selected".into(),
        binding: BoundLink {
            binding_id: "binding/selected".into(),
            source: LinkEndpoint {
                host_id: "host/source".into(),
                boot_id: "boot/source".into(),
                endpoint_id: "endpoint/source".into(),
            },
            sink: LinkEndpoint {
                host_id: "host/sink".into(),
                boot_id: "boot/sink".into(),
                endpoint_id: "endpoint/sink".into(),
            },
            base: "conduit.base/websocket-rfc6455@1".into(),
            base_instance_id: "base/selected".into(),
            credential: LinkCredentialReference::Opaque("credential/selected".into()),
            authority: LinkAuthorityReference::Grant("grant/selected".into()),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 8,
                maximum_buffered_bytes: 8,
                maximum_frame_bytes: 8,
            },
        },
        contract: LineContract {
            scope: LineScope::Machine,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::PlaintextNetwork,
        },
    }
}

/// Reuse the admitted state fixture's exact typed ports, isolating connection
/// occurrence laws from whole-Plan fingerprint admission in the private seam.
pub fn plan(cross_host: bool, cords: usize) -> Plan {
    let mut fragment = state::fragment();
    fragment.states.clear();
    let mut source = fragment.placements[0].clone();
    source.placement_id = "source".into();
    source.host_id = "host/source".into();
    source.boot_id = "boot/source".into();
    source.inputs.clear();
    let mut sink = fragment.placements[0].clone();
    sink.placement_id = "sink".into();
    sink.host_id = if cross_host {
        "host/sink"
    } else {
        "host/source"
    }
    .into();
    sink.boot_id = if cross_host {
        "boot/sink"
    } else {
        "boot/source"
    }
    .into();
    sink.outputs.clear();
    fragment.host_id = source.host_id.clone();
    fragment.boot_id = source.boot_id.clone();
    fragment.startup_order = vec![source.placement_id.clone()];
    fragment.placements = vec![source];
    fragment.connections = (0..cords)
        .rev()
        .map(|i| PlannedConnection {
            connection_id: format!("cord/{i:03}").into(),
            source_placement_id: "source".into(),
            source_port_id: "current".into(),
            sink_placement_id: "sink".into(),
            sink_port_id: "next".into(),
            value_kind: "fixture/byte@1".into(),
            resource: None,
            track: ConnectionTrack::Payload,
            temporal: PortTemporal::Value,
            pressure_policy: DeliveryPressurePolicy::PreserveOrder,
            selected_line: cross_host.then(line),
            admitted_lines: if cross_host { vec![line()] } else { vec![] },
            item_capacity: 1,
            byte_capacity: 8,
            abnormal_kind: None,
        })
        .collect();
    if !cross_host {
        fragment.startup_order.push(sink.placement_id.clone());
        fragment.placements.push(sink);
        return state::seal(fragment);
    }
    let mut destination = fragment.clone();
    destination.host_id = sink.host_id.clone();
    destination.boot_id = sink.boot_id.clone();
    destination.startup_order = vec![sink.placement_id.clone()];
    destination.placements = vec![sink];
    let mut plan = state::seal(fragment);
    plan.fragments.push(destination);
    plan
}

pub fn change_connection(plan: &mut Plan, mut change: impl FnMut(&mut PlannedConnection)) {
    for fragment in &mut plan.fragments {
        for connection in &mut fragment.connections {
            change(connection);
        }
    }
}
