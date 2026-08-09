//! Exact Session binding reconstructed from the generated Pico network image.

use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
    HostId, KindId, LineId, LinkBindingId, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_wire::{
    LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits,
};

use crate::network_image::generated_remote_endpoint;
use crate::receipts::RuntimeTranscriptIdentity;
use crate::usb_link::UsbLinkError;

pub fn session_binding(
    runtime: &RuntimeTranscriptIdentity,
) -> Result<SessionBinding, UsbLinkError> {
    let endpoint = generated_remote_endpoint().ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
    let base = ConnectionBase::from_canonical_code(endpoint.base_code)
        .ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
    if base != ConnectionBase::UsbCdc {
        return Err(UsbLinkError::InvalidGeneratedEndpoint);
    }
    let plan_id = PlanId::from(crate::network_image::PLAN_ID);
    let source_host = HostId::from(endpoint.peer_host);
    let source_boot = BootId::from(endpoint.peer_boot);
    let sink_host = HostId::from(endpoint.local_host);
    let sink_boot = BootId::from(endpoint.local_boot);
    SessionBinding {
        protocol_version: 1,
        source_active_play_id: bind_active_play(&plan_id, &source_host, &source_boot, 0).active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink_host, &sink_boot, 0).active_play_id,
        plan_id,
        source_fragment_id: FragmentId::from(endpoint.source_fragment_id),
        sink_fragment_id: FragmentId::from(endpoint.sink_fragment_id),
        connection_id: ConnectionId::from(endpoint.connection_id),
        source: SessionEndpointIdentity { host_id: source_host.clone(), boot_id: source_boot.clone() },
        sink: SessionEndpointIdentity { host_id: sink_host.clone(), boot_id: sink_boot.clone() },
        value_kind: KindId::from(endpoint.value_kind),
        limits: SessionLimits {
            maximum_in_flight_items: endpoint.maximum_in_flight_items,
            maximum_payload_bytes: endpoint.maximum_payload_bytes,
            maximum_buffered_bytes: endpoint.maximum_buffered_bytes,
        },
        attachment: LineAttachment {
            line_id: LineId::from(endpoint.line_id),
            link_binding_id: LinkBindingId::from(endpoint.link_binding_id),
            base,
            base_instance_id: ConnectionBaseInstanceId::from(endpoint.base_instance_id),
            source_host_id: source_host,
            source_boot_id: source_boot,
            source_endpoint_id: LinkEndpointId::from(endpoint.peer_endpoint),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from(endpoint.local_endpoint),
            limits: LinkLimits {
                maximum_in_flight_items: endpoint.maximum_in_flight_items,
                maximum_payload_bytes: endpoint.maximum_payload_bytes,
                maximum_buffered_bytes: endpoint.maximum_buffered_bytes,
                maximum_frame_bytes: endpoint.maximum_frame_bytes,
            },
        },
    }
    .with_observed_boots(BootId::from(endpoint.peer_boot), BootId::from(runtime.boot_id()))
    .map_err(UsbLinkError::Codec)
}
