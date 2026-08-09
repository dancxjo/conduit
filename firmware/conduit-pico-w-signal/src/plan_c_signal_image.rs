//! Immutable generated dual-Line Plan C identity and endpoints.

mod generated {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/r1_plan_c_signal_image.rs"));
}

use conduit_core::ConnectionBase;
use conduit_kernel::{CordId, RemoteEndpointId};

use crate::signal_execution_identity::SignalExecutionIdentity;
use crate::signal_image::RemoteEndpointIdentity;

pub fn execution_identity() -> SignalExecutionIdentity {
    SignalExecutionIdentity {
        firmware_build_id: generated::FIRMWARE_BUILD_ID,
        source_document_id: generated::SOURCE_DOCUMENT_ID,
        checked_form_id: generated::CHECKED_FORM_ID,
        expanded_form_id: generated::EXPANDED_FORM_ID,
        plan_id: generated::PLAN_ID,
        fragment_id: generated::FRAGMENT_ID,
        host_id: generated::HOST_ID,
        boot_id: generated::BOOT_ID,
        active_play_id: generated::ACTIVE_PLAY_ID,
        terminal_clue_id: generated::TERMINAL_CLUE_ID,
        presentation_ids: &generated::PRESENTATION_IDS,
        presentation_clue_ids: &generated::PRESENTATION_CLUE_IDS,
    }
}

pub fn endpoint(base: ConnectionBase) -> Option<RemoteEndpointIdentity> {
    if generated::GENERATED_REMOTE_ENDPOINT_COUNT != 2 {
        return None;
    }
    let index = generated::GENERATED_REMOTE_ENDPOINT_BASE_CODES
        .iter()
        .position(|code| ConnectionBase::from_canonical_code(*code) == Some(base))?;
    let cord = CordId(*generated::GENERATED_REMOTE_ENDPOINT_CORDS.get(index)?);
    let cord_spec = generated::GENERATED_CORDS.get(usize::from(cord.0))?;
    Some(RemoteEndpointIdentity {
        endpoint: RemoteEndpointId(*generated::GENERATED_REMOTE_ENDPOINT_IDS.get(index)?),
        cord,
        connection_id: generated::GENERATED_REMOTE_ENDPOINT_CONNECTION_IDS.get(index)?,
        source_fragment_id: generated::GENERATED_REMOTE_ENDPOINT_SOURCE_FRAGMENT_IDS.get(index)?,
        sink_fragment_id: generated::GENERATED_REMOTE_ENDPOINT_SINK_FRAGMENT_IDS.get(index)?,
        local_host: generated::GENERATED_REMOTE_ENDPOINT_LOCAL_HOSTS.get(index)?,
        local_boot: generated::GENERATED_REMOTE_ENDPOINT_LOCAL_BOOTS.get(index)?,
        local_endpoint: generated::GENERATED_REMOTE_ENDPOINT_LOCAL_ENDPOINTS.get(index)?,
        peer_host: generated::GENERATED_REMOTE_ENDPOINT_PEER_HOSTS.get(index)?,
        peer_boot: generated::GENERATED_REMOTE_ENDPOINT_PEER_BOOTS.get(index)?,
        peer_endpoint: generated::GENERATED_REMOTE_ENDPOINT_PEER_ENDPOINTS.get(index)?,
        base_code: *generated::GENERATED_REMOTE_ENDPOINT_BASE_CODES.get(index)?,
        base_instance_id: generated::GENERATED_REMOTE_ENDPOINT_BASE_INSTANCE_IDS.get(index)?,
        line_id: generated::GENERATED_REMOTE_ENDPOINT_LINE_IDS.get(index)?,
        link_binding_id: generated::GENERATED_REMOTE_ENDPOINT_LINK_BINDING_IDS.get(index)?,
        value_kind: generated::GENERATED_REMOTE_ENDPOINT_VALUE_KINDS.get(index)?,
        session_item_capacity: cord_spec.item_capacity,
        session_byte_capacity: cord_spec.byte_capacity,
        maximum_in_flight_items: *generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_IN_FLIGHT_ITEMS
            .get(index)?,
        maximum_payload_bytes: *generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_PAYLOAD_BYTES
            .get(index)?,
        maximum_buffered_bytes: *generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_BUFFERED_BYTES
            .get(index)?,
        maximum_frame_bytes: *generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_FRAME_BYTES.get(index)?,
    })
}

pub fn validate() -> bool {
    let identity = execution_identity();
    let Some(websocket) = endpoint(ConnectionBase::WebSocket) else {
        return false;
    };
    let Some(usb) = endpoint(ConnectionBase::UsbCdc) else {
        return false;
    };
    websocket.connection_id == usb.connection_id
        && websocket.source_fragment_id == usb.source_fragment_id
        && websocket.sink_fragment_id == usb.sink_fragment_id
        && websocket.local_host == usb.local_host
        && websocket.local_boot == usb.local_boot
        && websocket.peer_host == usb.peer_host
        && websocket.peer_boot == usb.peer_boot
        && websocket.value_kind == usb.value_kind
        && websocket.local_host == identity.host_id
        && websocket.local_boot == identity.boot_id
        && websocket.sink_fragment_id == identity.fragment_id
        && websocket.link_binding_id != usb.link_binding_id
        && websocket.base_instance_id != usb.base_instance_id
        && websocket.endpoint != usb.endpoint
}
