//! Exact R1 USB and WebSocket route facts for one observed Pico boot.

use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
    HostId, KindId, LinkAuthorityReference, LinkAvailability, LinkBinding, LinkBindingId,
    LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId, PROTOCOL_VERSION,
};
use conduit_wire::{RouteAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits};

pub const R1_STD_HOST_ID: &str = "r1/std-bootstrap";
pub const R1_STD_BOOT_ID: &str = "r1/std-bootstrap-boot";
pub const R1_PICO_HOST_ID: &str = "r1/pico-w";
pub const R1_PICO_BOOT_ID: &str = "r1/pico-w-boot";
pub const R1_USB_LINK_BINDING_ID: &str = "r1/std-pico-usb-bootstrap";
pub const R1_USB_BASE_INSTANCE_ID: &str = "r1/pico-usb-cdc-0";
pub const R1_STD_USB_ENDPOINT_ID: &str = "r1/std-usb-egress";
pub const R1_PICO_USB_ENDPOINT_ID: &str = "r1/pico-usb-ingress";
pub const R1_WEBSOCKET_LINK_BINDING_ID: &str = "r1/std-pico-websocket-route";
pub const R1_WEBSOCKET_BASE_INSTANCE_ID: &str = "r1/pico-websocket-0";
pub const R1_STD_WEBSOCKET_ENDPOINT_ID: &str = "r1/std-websocket-egress";
pub const R1_PICO_WEBSOCKET_ENDPOINT_ID: &str = "r1/pico-websocket-ingress";
pub const R1_WEBSOCKET_PORT: u16 = 8_765;
pub const R1_MAXIMUM_FRAME_BYTES: u32 = 2_048;
pub const R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES: u32 = 16;
pub const R1_WEBSOCKET_ROUTE_CLUE_ID: &str = "r1/pico-websocket-route-ready";
pub const R1_WEBSOCKET_PROBE_PLAN_ID: &str = "r1/websocket-route-probe-plan";
pub const R1_WEBSOCKET_PROBE_CONNECTION_ID: &str = "r1/websocket-route-probe-connection";
pub const R1_WEBSOCKET_PROBE_KIND: &str = "network/link-probe";

pub const R1_WEBSOCKET_BASE_QUERY: &[u8] = b"CONDUIT_R1_WEBSOCKET_BASE_QUERY@1";
pub const R1_PLAN_C_WEBSOCKET_BASE_QUERY: &[u8] = b"CONDUIT_R1_PLAN_C_WEBSOCKET_BASE_QUERY@1";
pub const R1_WEBSOCKET_BASE_READY: &[u8] = b"CONDUIT_R1_WEBSOCKET_BASE_READY@1";
pub const R1_WEBSOCKET_ENDPOINT_CLUE_READY: &[u8] = b"CONDUIT_R1_WEBSOCKET_ENDPOINT_CLUE_READY@1";

pub fn r1_websocket_link(pico_boot_id: BootId) -> LinkBinding {
    link(
        ConnectionBase::WebSocket,
        R1_WEBSOCKET_LINK_BINDING_ID,
        R1_WEBSOCKET_BASE_INSTANCE_ID,
        R1_STD_WEBSOCKET_ENDPOINT_ID,
        R1_PICO_WEBSOCKET_ENDPOINT_ID,
        pico_boot_id,
    )
}

pub fn r1_usb_link_for_boot(pico_boot_id: BootId) -> LinkBinding {
    link(
        ConnectionBase::UsbCdc,
        R1_USB_LINK_BINDING_ID,
        R1_USB_BASE_INSTANCE_ID,
        R1_STD_USB_ENDPOINT_ID,
        R1_PICO_USB_ENDPOINT_ID,
        pico_boot_id,
    )
}

pub fn r1_route_basis(pico_boot_id: BootId) -> [LinkBinding; 2] {
    [
        r1_usb_link_for_boot(pico_boot_id.clone()),
        r1_websocket_link(pico_boot_id),
    ]
}

pub fn r1_websocket_probe_binding(pico_boot_id: BootId) -> SessionBinding {
    let link = r1_websocket_link(pico_boot_id);
    let plan_id = PlanId::from(R1_WEBSOCKET_PROBE_PLAN_ID);
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        source_active_play_id: bind_active_play(
            &plan_id,
            &link.source.host_id,
            &link.source.boot_id,
            0,
        )
        .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &link.sink.host_id, &link.sink.boot_id, 0)
            .active_play_id,
        plan_id,
        source_fragment_id: FragmentId::from("r1/websocket-route-probe-source"),
        sink_fragment_id: FragmentId::from("r1/websocket-route-probe-sink"),
        connection_id: ConnectionId::from(R1_WEBSOCKET_PROBE_CONNECTION_ID),
        source: SessionEndpointIdentity {
            host_id: link.source.host_id.clone(),
            boot_id: link.source.boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: link.sink.host_id.clone(),
            boot_id: link.sink.boot_id.clone(),
        },
        value_kind: KindId::from(R1_WEBSOCKET_PROBE_KIND),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES,
            maximum_buffered_bytes: R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES,
        },
        attachment: RouteAttachment {
            link_binding_id: link.binding_id,
            base: link.base,
            base_instance_id: link.base_instance_id,
            source_host_id: link.source.host_id,
            source_boot_id: link.source.boot_id,
            source_endpoint_id: link.source.endpoint_id,
            sink_host_id: link.sink.host_id,
            sink_boot_id: link.sink.boot_id,
            sink_endpoint_id: link.sink.endpoint_id,
            limits: link.limits,
        },
    }
}

fn link(
    base: ConnectionBase,
    binding_id: &str,
    base_instance_id: &str,
    source_endpoint_id: &str,
    sink_endpoint_id: &str,
    pico_boot_id: BootId,
) -> LinkBinding {
    LinkBinding {
        binding_id: LinkBindingId::from(binding_id),
        source: LinkEndpoint {
            host_id: HostId::from(R1_STD_HOST_ID),
            boot_id: BootId::from(R1_STD_BOOT_ID),
            endpoint_id: LinkEndpointId::from(source_endpoint_id),
        },
        sink: LinkEndpoint {
            host_id: HostId::from(R1_PICO_HOST_ID),
            boot_id: pico_boot_id,
            endpoint_id: LinkEndpointId::from(sink_endpoint_id),
        },
        base,
        base_instance_id: ConnectionBaseInstanceId::from(base_instance_id),
        availability: LinkAvailability::Ready,
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::ProcessOwned,
        limits: LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES,
            maximum_buffered_bytes: R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES,
            maximum_frame_bytes: R1_MAXIMUM_FRAME_BYTES,
        },
    }
}
