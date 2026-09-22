use conduit_protected_line::{
    EndpointBinding, ProtectedSessionPolicy, RelayCandidateBounds, RelayCandidateDescriptor,
    RelayCandidateError, RelayCandidateIdentity, RelayCandidateSecrets, RelayEndpointRole,
    SessionBinding, SessionLimits, RELAY_CANDIDATE_SCHEMA, RELAY_SERVICE_IMPLEMENTATION_ID,
};

fn candidate(now: u64) -> RelayCandidateDescriptor {
    RelayCandidateDescriptor::new(
        RelayCandidateIdentity {
            schema: RELAY_CANDIDATE_SCHEMA.into(),
            relay_implementation_id: RELAY_SERVICE_IMPLEMENTATION_ID.into(),
            relay_locator: "wss://relay.example/conduit".into(),
            relay_server_identity: "relay.example".into(),
            certificate_binding_sha256: [1; 32],
            negotiation_id: "negotiation/one".into(),
            route_id: "route/one".into(),
            role: RelayEndpointRole::First,
            endpoint_binding: "host/one/boot/one".into(),
            session_binding: SessionBinding {
                initiator: EndpointBinding {
                    host_id: "host/one".into(),
                    boot_id: "boot/one".into(),
                },
                responder: EndpointBinding {
                    host_id: "host/two".into(),
                    boot_id: "boot/two".into(),
                },
                negotiation_id: "negotiation/one".into(),
                line_session_id: "line/one".into(),
                candidate_binding: "route/one".into(),
                transport_binding: "relay/wss/certificate/one".into(),
            },
            expires_at_millis: now + 10_000,
        },
        RelayCandidateBounds {
            maximum_protected_frame_bytes: 290,
            maximum_attempts: 1,
            attempt_timeout_millis: 2_000,
            protected_session: ProtectedSessionPolicy {
                traffic: SessionLimits {
                    maximum_payload_bytes: 256,
                    maximum_frames_per_direction: 8,
                    maximum_bytes_per_direction: 2_048,
                },
                maximum_simultaneous_sessions: 1,
                maximum_pending_frames_per_session: 1,
                handshake_work_units: 2,
                handshake_timeout_millis: 2_000,
                idle_timeout_millis: 5_000,
            },
        },
        RelayCandidateSecrets::new([7; 32], [8; 32]),
        now,
    )
    .unwrap()
}

#[test]
fn one_portable_candidate_binds_relay_peer_session_and_every_bound() {
    let candidate = candidate(1_000);
    assert_eq!(candidate.validate(1_000), Ok(()));
    assert_eq!(candidate.copy_relay_capability_for_attempt(), [7; 32]);
    assert_eq!(candidate.copy_protected_session_psk_for_attempt(), [8; 32]);
    let debug = format!("{candidate:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("7, 7, 7"));
}

#[test]
fn stale_mismatched_and_underbounded_candidates_refuse_before_transport() {
    let stale = candidate(1_000);
    assert_eq!(stale.validate(11_000), Err(RelayCandidateError::Expired));

    let mut mismatched = candidate(1_000);
    mismatched.identity.session_binding.candidate_binding = "route/substitute".into();
    assert_eq!(
        mismatched.validate(1_000),
        Err(RelayCandidateError::BindingMismatch)
    );

    let mut underbounded = candidate(1_000);
    underbounded.bounds.maximum_protected_frame_bytes = 289;
    assert_eq!(
        underbounded.validate(1_000),
        Err(RelayCandidateError::InvalidBounds)
    );
}
