//! Exact refusal for a protected Line profile that ConduitOS cannot yet realize.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectedLineRefusal {
    pub implementation_id: &'static str,
    pub code: &'static str,
    pub detail: &'static str,
}

/// ConduitOS has no admitted entropy source or compiled Noise realization yet.
/// It must not advertise the portable profile merely because its contract types
/// compile for the target.
pub const fn require_protected_line() -> Result<(), ProtectedLineRefusal> {
    Err(ProtectedLineRefusal {
        implementation_id: conduit_protected_line::IMPLEMENTATION_ID,
        code: "protected-line-implementation-unavailable",
        detail: "ConduitOS has no admitted entropy source and Noise session realization",
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayCandidateRefusal {
    Candidate(conduit_protected_line::RelayCandidateError),
    ProtectedLine(ProtectedLineRefusal),
}

/// Consume the shared relay-candidate contract without pretending this image
/// has the entropy, Noise, TLS, or socket realization needed to execute it.
pub fn require_relay_candidate(
    candidate: &conduit_protected_line::RelayCandidateDescriptor,
    now_millis: u64,
) -> Result<(), RelayCandidateRefusal> {
    candidate
        .validate(now_millis)
        .map_err(RelayCandidateRefusal::Candidate)?;
    require_protected_line().map_err(RelayCandidateRefusal::ProtectedLine)
}

#[cfg(test)]
mod tests {
    use conduit_protected_line::{
        EndpointBinding, ProtectedSessionPolicy, RELAY_CANDIDATE_SCHEMA,
        RELAY_SERVICE_IMPLEMENTATION_ID, RelayCandidateBounds, RelayCandidateDescriptor,
        RelayCandidateIdentity, RelayCandidateSecrets, RelayEndpointRole, SessionBinding,
        SessionLimits,
    };

    #[test]
    fn conduitos_refuses_instead_of_advertising_false_protection() {
        let refusal = super::require_protected_line().unwrap_err();
        assert_eq!(
            refusal.implementation_id,
            "conduit.line/noise-nnpsk0-25519-chachapoly-sha256@1"
        );
        assert_eq!(refusal.code, "protected-line-implementation-unavailable");
    }

    #[test]
    fn conduitos_consumes_the_portable_relay_candidate_then_refuses_truthfully() {
        let candidate = RelayCandidateDescriptor::new(
            RelayCandidateIdentity {
                schema: RELAY_CANDIDATE_SCHEMA.into(),
                relay_implementation_id: RELAY_SERVICE_IMPLEMENTATION_ID.into(),
                relay_locator: "wss://relay.example/conduit".into(),
                relay_server_identity: "relay.example".into(),
                certificate_binding_sha256: [1; 32],
                negotiation_id: "negotiation/one".into(),
                route_id: "route/one".into(),
                role: RelayEndpointRole::Second,
                endpoint_binding: "host/conduitos/boot/one".into(),
                session_binding: SessionBinding {
                    initiator: EndpointBinding {
                        host_id: "host/native".into(),
                        boot_id: "boot/native".into(),
                    },
                    responder: EndpointBinding {
                        host_id: "host/conduitos".into(),
                        boot_id: "boot/one".into(),
                    },
                    negotiation_id: "negotiation/one".into(),
                    line_session_id: "line/one".into(),
                    candidate_binding: "route/one".into(),
                    transport_binding: "relay/wss/certificate/one".into(),
                },
                expires_at_millis: 10_000,
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
            1_000,
        )
        .unwrap();
        let refusal = super::require_relay_candidate(&candidate, 1_000).unwrap_err();
        assert!(matches!(
            refusal,
            super::RelayCandidateRefusal::ProtectedLine(reason)
                if reason.code == "protected-line-implementation-unavailable"
        ));
    }
}
