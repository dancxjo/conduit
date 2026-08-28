use conduit_core::{
    AdmittedLine, BaseImplementationId, BaseInstanceId, BootId, BoundLink, ConnectionId,
    FragmentId, HostId, KindId, LineContinuation, LineContract, LineDuplex, LineId, LineOrdering,
    LineReliability, LineScope, LineSecurity, LineTrafficShape, LinkAuthorityReference,
    LinkBindingId, LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits, PlacementId,
    PlannedConnection, PortId, PortTemporal,
};
use conduit_wire::{SessionBinding, WireError};

use super::{FRAME_CAPACITY, PAYLOAD_CAPACITY};

pub(super) fn exact_binding(variant: u32) -> Result<SessionBinding, WireError> {
    let mut plan_id = conduit_core::PlanId::from("browser-webrtc/plan/1");
    let source = LinkEndpoint {
        host_id: HostId::from("browser-webrtc/source"),
        boot_id: BootId::from("browser-webrtc/source-boot/1"),
        endpoint_id: LinkEndpointId::from("browser-webrtc/source-egress"),
    };
    let sink = LinkEndpoint {
        host_id: HostId::from("browser-webrtc/sink"),
        boot_id: BootId::from("browser-webrtc/sink-boot/1"),
        endpoint_id: LinkEndpointId::from("browser-webrtc/sink-ingress"),
    };
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: PAYLOAD_CAPACITY,
        maximum_buffered_bytes: PAYLOAD_CAPACITY,
        maximum_frame_bytes: FRAME_CAPACITY as u32,
    };
    let line = AdmittedLine {
        line_id: LineId::from("browser-webrtc/line/1"),
        binding: BoundLink {
            binding_id: LinkBindingId::from("browser-webrtc/binding/1"),
            source: source.clone(),
            sink: sink.clone(),
            base: BaseImplementationId::from("conduit.base/webrtc-data-channel@1"),
            base_instance_id: BaseInstanceId::from("browser-webrtc/base-instance/1"),
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits,
        },
        contract: LineContract {
            scope: LineScope::PointToPoint,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::AuthenticatedEncrypted,
        },
    };
    let mut connection = PlannedConnection {
        connection_id: ConnectionId::from("browser-webrtc/connection/1"),
        source_placement_id: PlacementId::from("browser-webrtc/source-placement"),
        source_port_id: PortId::from("out"),
        sink_placement_id: PlacementId::from("browser-webrtc/sink-placement"),
        sink_port_id: PortId::from("in"),
        value_kind: KindId::from("conduit-test/bounded-bytes@1"),
        temporal: PortTemporal::Value,
        selected_line: Some(line.clone()),
        admitted_lines: vec![line],
        item_capacity: 1,
        byte_capacity: PAYLOAD_CAPACITY,
    };
    match variant {
        0 => {}
        1 => connection.connection_id = ConnectionId::from("browser-webrtc/wrong-connection"),
        2 => connection.value_kind = KindId::from("conduit-test/wrong-value@1"),
        3 => {
            let line = connection.selected_line.as_mut().expect("selected Line");
            line.binding.source.boot_id = BootId::from("browser-webrtc/stale-source-boot");
            connection.admitted_lines[0] = line.clone();
        }
        4 => {
            let line = connection.selected_line.as_mut().expect("selected Line");
            line.binding.base_instance_id =
                BaseInstanceId::from("browser-webrtc/wrong-base-instance");
            connection.admitted_lines[0] = line.clone();
        }
        5 => plan_id = conduit_core::PlanId::from("browser-webrtc/wrong-plan"),
        _ => return Err(WireError::InvalidSession),
    }
    SessionBinding::from_planned_connection(
        plan_id,
        FragmentId::from("browser-webrtc/source-fragment"),
        FragmentId::from("browser-webrtc/sink-fragment"),
        &connection,
    )
}
