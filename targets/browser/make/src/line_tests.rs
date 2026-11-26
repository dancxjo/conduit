//! Exact realization inventory; legacy and explicit full-Face bounds stay distinct.
use super::*;
use std::collections::BTreeSet;

#[test]
fn line_realizations_bind_exact_contracts_limits_and_outbound_authority() {
    assert_eq!(BROWSER_LINE_REALIZATIONS.len(), 3);
    let made = BROWSER_IMPLEMENTATIONS
        .iter()
        .map(|item| item.implementation_id)
        .collect::<BTreeSet<_>>();
    for line in BROWSER_LINE_REALIZATIONS {
        assert!(made.contains(line.make_implementation_id));
        assert_eq!(line.implementation_revision, 1);
        assert!(line.base_implementation_id.starts_with("conduit.base/"));
        assert!(!line.artifact_id.is_empty());
        assert_eq!(line.maximum_sessions_per_host, 4);
        assert_eq!(line.maximum_in_flight_items, 1);
        assert!(line.maximum_frame_bytes >= line.maximum_payload_bytes);
        assert!(line.maximum_frame_bytes <= line.maximum_buffered_bytes);
        assert!(line.maximum_received_messages > 0);
        assert!(!line.endpoint_authority.is_empty());
        assert!(!line.credential_requirement.is_empty());
        assert!(line.initiates_outbound_only);
    }
    for legacy in &BROWSER_LINE_REALIZATIONS[..2] {
        assert!(legacy.artifact_id.ends_with(".mjs@1"));
        assert_eq!(legacy.maximum_payload_bytes, 64 * 1024);
        assert_eq!(legacy.maximum_buffered_bytes, 256 * 1024);
    }
    assert_eq!(BROWSER_LINE_REALIZATIONS[2], OWNER_FACE_FULL_LINE);
    assert_eq!(
        OWNER_FACE_FULL_LINE.maximum_payload_bytes as usize,
        OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES as usize
    );
    assert_eq!(
        OWNER_FACE_FULL_LINE.maximum_frame_bytes as usize,
        OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES as usize
    );
    assert_eq!(
        OWNER_FACE_FULL_LINE.maximum_buffered_bytes as usize,
        4 * OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES as usize
    );
    let websocket = &BROWSER_LINE_REALIZATIONS[0];
    assert_eq!(
        websocket.contract.scope,
        conduit_core::LineScope::RoutedNetwork
    );
    assert_eq!(
        websocket.contract.security,
        conduit_core::LineSecurity::PlaintextNetwork
    );
    assert_eq!(websocket.maximum_frame_bytes, 64 * 1024);
    assert_eq!(websocket.signaling_bootstrap, None);
    let webrtc = &BROWSER_LINE_REALIZATIONS[1];
    assert_eq!(webrtc.contract.scope, conduit_core::LineScope::PointToPoint);
    assert_eq!(
        webrtc.contract.security,
        conduit_core::LineSecurity::AuthenticatedEncrypted
    );
    assert_eq!(webrtc.maximum_frame_bytes, 128 * 1024);
    assert!(webrtc.signaling_bootstrap.is_some());
}
