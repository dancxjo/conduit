use conduit_core::{
    LineContinuation, LineContract, LineDuplex, LineOrdering, LineReliability, LineScope,
    LineSecurity, LineTrafficShape,
};

/// Exact reviewed binding from a selectable browser make offer to the
/// portable Line contract and finite runtime realization that implements it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserLineRealizationDescriptor {
    pub make_implementation_id: &'static str,
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

/// Explicit large owner Face realization. These bounds do not change either
/// legacy Line. The response keeps the ordinary Face and route JSON values.
pub const OWNER_FACE_FULL_MAXIMUM_FACE_BYTES: u32 = 512 * 1024;
pub const OWNER_FACE_FULL_MAXIMUM_ROUTE_BYTES: u32 = 64 * 1024;
const RESPONSE_OVERHEAD:u32=br#"{"outcome":"snapshot","schema":"conduit.face/owner-face-response@1","face":,"interactions_admitted":false,"route":}"#.len() as u32;
const FRAME_OVERHEAD: u32 =
    br#"{"kind":"face-snapshot-response","protocol":65535,"response":}"#.len() as u32;
pub const OWNER_FACE_FULL_MAXIMUM_RESPONSE_BYTES: u32 =
    OWNER_FACE_FULL_MAXIMUM_FACE_BYTES + OWNER_FACE_FULL_MAXIMUM_ROUTE_BYTES + RESPONSE_OVERHEAD;
pub const OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES: u32 =
    OWNER_FACE_FULL_MAXIMUM_RESPONSE_BYTES + FRAME_OVERHEAD;
/// Independent selected output profiles; complete oversized outputs refuse.
pub const OWNER_FACE_FULL_MAXIMUM_VIEW_BYTES: u32 = 1024 * 1024;
pub const OWNER_FACE_FULL_MAXIMUM_CANONICAL_BYTES: u32 = 512 * 1024;
pub const OWNER_FACE_FULL_LINE: BrowserLineRealizationDescriptor =
    BrowserLineRealizationDescriptor {
        make_implementation_id: "browser/websocket-owner-face@1",
        implementation_revision: 1,
        base_implementation_id: "conduit.base/websocket-rfc6455@1",
        artifact_id: "browser-host/browser-membership.js@owner-face-full1",
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
        maximum_payload_bytes: OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES,
        maximum_frame_bytes: OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES,
        maximum_buffered_bytes: 4 * OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES,
        maximum_received_messages: 1,
        endpoint_authority: "runtime endpoint grant",
        credential_requirement: "none or opaque runtime credential reference",
        signaling_bootstrap: None,
        initiates_outbound_only: true,
    };

pub const BROWSER_LINE_REALIZATIONS: &[BrowserLineRealizationDescriptor] = &[
    BrowserLineRealizationDescriptor {
        make_implementation_id: "browser/websocket@1",
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
        make_implementation_id: "browser/webrtc-datachannel@1",
        implementation_revision: 1,
        base_implementation_id: "conduit.base/webrtc-data-channel@1",
        artifact_id: "browser-host/webrtc-datachannel-line.mjs@1",
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
        endpoint_authority: "body-scoped runtime grant",
        credential_requirement: "Body-grant-scoped session credential",
        signaling_bootstrap: Some("body-scoped signaling session"),
        initiates_outbound_only: true,
    },
    OWNER_FACE_FULL_LINE,
];

#[cfg(test)]
mod owner_face_full_tests {
    use super::*;
    #[test]
    fn full_realization_is_additive_and_funds_complete_envelope() {
        assert_eq!(
            BROWSER_LINE_REALIZATIONS[0].maximum_payload_bytes,
            64 * 1024
        );
        assert_eq!(
            BROWSER_LINE_REALIZATIONS[1].maximum_payload_bytes,
            64 * 1024
        );
        assert_eq!(
            OWNER_FACE_FULL_LINE.maximum_payload_bytes,
            OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES
        );
        assert_eq!(
            OWNER_FACE_FULL_LINE.maximum_buffered_bytes,
            4 * OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES
        );
        const {
            assert!(
                OWNER_FACE_FULL_LINE.maximum_payload_bytes
                    > OWNER_FACE_FULL_MAXIMUM_FACE_BYTES + OWNER_FACE_FULL_MAXIMUM_ROUTE_BYTES
            );
        }
        assert_ne!(
            OWNER_FACE_FULL_LINE.make_implementation_id,
            BROWSER_LINE_REALIZATIONS[0].make_implementation_id
        );
        assert_eq!(
            OWNER_FACE_FULL_LINE.base_implementation_id,
            BROWSER_LINE_REALIZATIONS[0].base_implementation_id
        );
    }
}
