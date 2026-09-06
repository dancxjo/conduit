use conduit_core::{
    LineContinuation, LineContract, LineDuplex, LineOrdering, LineReliability, LineScope,
    LineSecurity, LineTrafficShape,
};

/// Exact reviewed binding from a selectable browser fabrication offer to the
/// portable Line contract and finite runtime realization that implements it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserLineRealizationDescriptor {
    pub fabrication_implementation_id: &'static str,
    pub implementation_revision: u32,
    pub base_implementation_id: &'static str,
    pub artifact_id: &'static str,
    pub contract: LineContract,
    pub maximum_sessions_per_host: u16,
    pub maximum_in_flight_items: u16,
    pub maximum_payload_bytes: u32,
    pub maximum_frame_bytes: u32,
    pub maximum_buffered_bytes: u32,
    pub maximum_received_messages: u16,
    pub endpoint_authority: &'static str,
    pub credential_requirement: &'static str,
    pub signaling_bootstrap: Option<&'static str>,
    pub initiates_outbound_only: bool,
}

pub const BROWSER_LINE_REALIZATIONS: &[BrowserLineRealizationDescriptor] = &[
    BrowserLineRealizationDescriptor {
        fabrication_implementation_id: "browser/websocket@1",
        implementation_revision: 1,
        base_implementation_id: "conduit.base/websocket-rfc6455@1",
        artifact_id: "browser-host/websocket-line.mjs@1",
        contract: LineContract {
            scope: LineScope::RoutedNetwork,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::PlaintextNetwork,
        },
        maximum_sessions_per_host: 4,
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 64 * 1024,
        maximum_frame_bytes: 64 * 1024,
        maximum_buffered_bytes: 256 * 1024,
        maximum_received_messages: 1,
        endpoint_authority: "runtime endpoint grant",
        credential_requirement: "none or opaque runtime credential reference",
        signaling_bootstrap: None,
        initiates_outbound_only: true,
    },
    BrowserLineRealizationDescriptor {
        fabrication_implementation_id: "browser/webrtc-datachannel@1",
        implementation_revision: 1,
        base_implementation_id: "conduit.base/webrtc-data-channel@1",
        artifact_id: "patchbay-html/webrtc-datachannel-line.mjs@1",
        contract: LineContract {
            scope: LineScope::PointToPoint,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::AuthenticatedEncrypted,
        },
        maximum_sessions_per_host: 4,
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 64 * 1024,
        maximum_frame_bytes: 128 * 1024,
        maximum_buffered_bytes: 256 * 1024,
        maximum_received_messages: 16,
        endpoint_authority: "Body-scoped runtime grant",
        credential_requirement: "Body-grant-scoped session credential",
        signaling_bootstrap: Some("Body-scoped signaling session"),
        initiates_outbound_only: true,
    },
];
