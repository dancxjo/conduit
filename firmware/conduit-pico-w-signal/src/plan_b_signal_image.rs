//! Immutable generated USB Plan B identity and endpoint.

mod generated {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/r1_plan_b_signal_image.rs"));
}

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

pub fn remote_endpoint() -> Option<RemoteEndpointIdentity> {
    if generated::GENERATED_REMOTE_ENDPOINT_COUNT != 1 {
        return None;
    }
    Some(RemoteEndpointIdentity {
        endpoint: RemoteEndpointId(*generated::GENERATED_REMOTE_ENDPOINT_IDS.first()?),
        cord: CordId(*generated::GENERATED_REMOTE_ENDPOINT_CORDS.first()?),
        connection_id: generated::GENERATED_REMOTE_ENDPOINT_CONNECTION_IDS.first()?,
        source_fragment_id: generated::GENERATED_REMOTE_ENDPOINT_SOURCE_FRAGMENT_IDS.first()?,
        sink_fragment_id: generated::GENERATED_REMOTE_ENDPOINT_SINK_FRAGMENT_IDS.first()?,
        local_host: generated::GENERATED_REMOTE_ENDPOINT_LOCAL_HOSTS.first()?,
        local_boot: generated::GENERATED_REMOTE_ENDPOINT_LOCAL_BOOTS.first()?,
        local_endpoint: generated::GENERATED_REMOTE_ENDPOINT_LOCAL_ENDPOINTS.first()?,
        peer_host: generated::GENERATED_REMOTE_ENDPOINT_PEER_HOSTS.first()?,
        peer_boot: generated::GENERATED_REMOTE_ENDPOINT_PEER_BOOTS.first()?,
        peer_endpoint: generated::GENERATED_REMOTE_ENDPOINT_PEER_ENDPOINTS.first()?,
        base_code: *generated::GENERATED_REMOTE_ENDPOINT_BASE_CODES.first()?,
        base_instance_id: generated::GENERATED_REMOTE_ENDPOINT_BASE_INSTANCE_IDS.first()?,
        link_binding_id: generated::GENERATED_REMOTE_ENDPOINT_LINK_BINDING_IDS.first()?,
        value_kind: generated::GENERATED_REMOTE_ENDPOINT_VALUE_KINDS.first()?,
        maximum_in_flight_items: *generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_IN_FLIGHT_ITEMS
            .first()?,
        maximum_payload_bytes: *generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_PAYLOAD_BYTES
            .first()?,
        maximum_buffered_bytes: *generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_BUFFERED_BYTES
            .first()?,
        maximum_frame_bytes: *generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_FRAME_BYTES.first()?,
    })
}

pub fn validate_replacement() -> bool {
    let Some(endpoint) = remote_endpoint() else {
        return false;
    };
    let plan_a = SignalExecutionIdentity::plan_a();
    let plan_b = SignalExecutionIdentity::plan_b();
    conduit_core::ConnectionBase::from_canonical_code(endpoint.base_code)
        == Some(conduit_core::ConnectionBase::UsbCdc)
        && endpoint.local_host == plan_b.host_id
        && endpoint.local_boot == plan_b.boot_id
        && endpoint.sink_fragment_id == plan_b.fragment_id
        && plan_a.plan_id != plan_b.plan_id
        && plan_a.fragment_id != plan_b.fragment_id
        && plan_a.active_play_id != plan_b.active_play_id
        && plan_a.host_id == plan_b.host_id
        && plan_a.boot_id == plan_b.boot_id
        && plan_a.checked_form_id == plan_b.checked_form_id
}
