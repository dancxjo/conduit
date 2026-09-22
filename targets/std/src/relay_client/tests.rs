use super::*;

fn descriptor() -> RelayClientDescriptor {
    RelayClientDescriptor {
        address: "127.0.0.1:7443".parse().unwrap(),
        public_url: "wss://relay.example:7443/conduit".into(),
        server_identity: "relay.example".into(),
        certificate_binding_sha256: [1; 32],
        route_id: "route/one".into(),
        role: RelayEndpointRole::First,
        endpoint_binding: "host/one/boot/one".into(),
        capability: [7; 32],
        maximum_protected_frame_bytes: 1024,
        timeout_millis: 2_000,
    }
}

#[test]
fn descriptor_is_bounded_and_debug_redacts_capability() {
    let valid = descriptor();
    assert_eq!(valid.validate(), Ok(()));
    let debug = format!("{valid:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("7, 7, 7"));

    let mut invalid = descriptor();
    invalid.capability = [0; 32];
    assert_eq!(invalid.validate(), Err(RelayClientError::InvalidDescriptor));
    let mut invalid = descriptor();
    invalid.maximum_protected_frame_bytes = 65_554;
    assert_eq!(invalid.validate(), Err(RelayClientError::InvalidDescriptor));
}

#[test]
fn outcome_requires_exact_service_and_route_identity() {
    let paired = br#"{"schema":"conduit.relay/outcome@1","implementation_id":"conduit.relay/opaque-two-endpoint@1","route_id":"route/one","status":"paired"}"#;
    assert!(matches!(
        decode_outcome(paired, "route/one"),
        Ok(OutcomeStatus::Paired)
    ));
    assert!(matches!(
        decode_outcome(paired, "route/substitute"),
        Err(RelayClientError::Protocol)
    ));
    let pressure = br#"{"schema":"conduit.relay/outcome@1","implementation_id":"conduit.relay/opaque-two-endpoint@1","route_id":"route/one","status":"pressure","code":"queue-pressure"}"#;
    assert!(matches!(
        decode_outcome(pressure, "route/one"),
        Ok(OutcomeStatus::Pressure)
    ));
}
