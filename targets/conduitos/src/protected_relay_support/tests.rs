use super::*;

use alloc::string::ToString;
use conduit_protected_line::{
    EndpointBinding, ProtectedHandshake, ProtectedSessionPolicy, RelayCandidateBounds,
    RelayCandidateIdentity, RelayCandidateSecrets, SessionBinding, SessionLimits,
};

use crate::cryptographic_entropy::{EntropyProvider, EntropyRefusal};

const PSK: [u8; 32] = [0x31; 32];
const ROUTE: &str = "route/one";

struct FixtureEntropy;

impl CryptographicEntropySource for FixtureEntropy {
    fn provider(&self) -> EntropyProvider {
        EntropyProvider {
            base_id: "base/entropy/relay-fixture",
            provider_instance_id: "fixture/one",
            provider_generation: 1,
        }
    }

    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        output.fill(0x42);
        Ok(())
    }
}

struct InspectingRelaySocket {
    responder: ProtectedHandshake,
    pending: [u8; MAXIMUM_BINARY_MESSAGE_BYTES],
    pending_len: usize,
    attachments: usize,
    envelopes: usize,
    closes: usize,
}

impl BinaryWebSocketIo for InspectingRelaySocket {
    fn send_binary(&mut self, payload: &[u8]) -> Result<(), WebSocketError> {
        if payload.starts_with(b"{") {
            if payload == CLOSE_CONTROL {
                self.closes += 1;
                return Ok(());
            }
            let text =
                core::str::from_utf8(payload).map_err(|_| WebSocketError::UnexpectedFrame)?;
            if !text.contains(ATTACH_SCHEMA)
                || !text.contains(ROUTE)
                || !text.contains("\"role\":\"first\"")
            {
                return Err(WebSocketError::UnexpectedFrame);
            }
            self.attachments += 1;
            let paired = br#"{"schema":"conduit.relay/outcome@1","implementation_id":"conduit.relay/opaque-two-endpoint@1","route_id":"route/one","status":"paired"}"#;
            self.pending[..paired.len()].copy_from_slice(paired);
            self.pending_len = paired.len();
            return Ok(());
        }

        let envelope =
            decode_relay_envelope(payload, 290).map_err(|_| WebSocketError::UnexpectedFrame)?;
        if envelope.route_id != ROUTE {
            return Err(WebSocketError::UnexpectedFrame);
        }
        self.envelopes += 1;
        self.responder
            .read_message(envelope.protected_frame)
            .map_err(|_| WebSocketError::UnexpectedFrame)?;
        let mut response = [0_u8; 64];
        let response_len = self
            .responder
            .next_message_bytes()
            .map_err(|_| WebSocketError::UnexpectedFrame)?;
        self.responder
            .write_message(&mut response[..response_len])
            .map_err(|_| WebSocketError::UnexpectedFrame)?;
        self.pending_len = RelayEnvelope {
            route_id: ROUTE,
            protected_frame: &response[..response_len],
        }
        .encode(&mut self.pending)
        .map_err(|_| WebSocketError::UnexpectedFrame)?;
        Ok(())
    }

    fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, WebSocketError> {
        if self.pending_len == 0 || output.len() < self.pending_len {
            return Err(WebSocketError::FrameRead);
        }
        let length = self.pending_len;
        output[..length].copy_from_slice(&self.pending[..length]);
        self.pending[..length].fill(0);
        self.pending_len = 0;
        Ok(length)
    }
}

fn binding() -> SessionBinding {
    SessionBinding {
        initiator: EndpointBinding {
            host_id: "host/conduitos".to_string(),
            boot_id: "boot/conduitos".to_string(),
        },
        responder: EndpointBinding {
            host_id: "host/std".to_string(),
            boot_id: "boot/std".to_string(),
        },
        negotiation_id: "negotiation/one".to_string(),
        line_session_id: "line/one".to_string(),
        candidate_binding: ROUTE.to_string(),
        transport_binding: "relay/wss/certificate/one".to_string(),
    }
}

fn policy() -> ProtectedSessionPolicy {
    ProtectedSessionPolicy {
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
    }
}

fn candidate(certificate_binding_sha256: [u8; 32]) -> RelayCandidateDescriptor {
    RelayCandidateDescriptor::new(
        RelayCandidateIdentity {
            schema: conduit_protected_line::RELAY_CANDIDATE_SCHEMA.to_string(),
            relay_implementation_id: RELAY_SERVICE_IMPLEMENTATION_ID.to_string(),
            relay_locator: "wss://relay.example:8443/conduit".to_string(),
            relay_server_identity: "relay.example".to_string(),
            certificate_binding_sha256,
            negotiation_id: "negotiation/one".to_string(),
            route_id: ROUTE.to_string(),
            role: RelayEndpointRole::First,
            endpoint_binding: "host/conduitos/boot/conduitos".to_string(),
            session_binding: binding(),
            expires_at_millis: 10_000,
        },
        RelayCandidateBounds {
            maximum_protected_frame_bytes: 290,
            maximum_attempts: 1,
            attempt_timeout_millis: 2_000,
            protected_session: policy(),
        },
        RelayCandidateSecrets::new([7; 32], PSK),
        1_000,
    )
    .unwrap()
}

#[test]
fn candidate_binds_exact_wss_endpoint_and_certificate_before_network_work() {
    let certificate = b"fixture certificate";
    let descriptor = candidate(Sha256::digest(certificate).into());
    let endpoint = VirtioTcpEndpoint {
        guest_address: [10, 0, 2, 15],
        prefix_length: 24,
        gateway: [10, 0, 2, 2],
        remote_address: [10, 0, 2, 2],
        remote_port: 8443,
        local_port: 49_152,
    };
    assert_eq!(
        validate_transport_binding::<()>(&descriptor, endpoint, certificate),
        Ok(())
    );
    let wrong_endpoint = VirtioTcpEndpoint {
        remote_port: 443,
        ..endpoint
    };
    assert_eq!(
        validate_transport_binding::<()>(&descriptor, wrong_endpoint, certificate),
        Err(ConduitOsRelayRefusal::EndpointBinding)
    );
    assert_eq!(
        validate_transport_binding::<()>(&descriptor, endpoint, b"other certificate"),
        Err(ConduitOsRelayRefusal::CertificateBinding)
    );
}

#[test]
fn shared_candidate_attaches_and_carries_the_real_protected_handshake() {
    let descriptor = candidate([1; 32]);
    let binding = binding();
    let mut socket = InspectingRelaySocket {
        responder: ProtectedHandshake::new(
            Role::Responder,
            &binding,
            policy().traffic,
            PSK,
            [0x53; 32],
        )
        .unwrap(),
        pending: [0; MAXIMUM_BINARY_MESSAGE_BYTES],
        pending_len: 0,
        attachments: 0,
        envelopes: 0,
        closes: 0,
    };
    let carrier = attach(&mut socket, &descriptor, 2_000).unwrap();
    let mut entropy = CryptographicEntropyBase::<_, 1>::admit(FixtureEntropy).unwrap();
    let mut protected = establish_conduitos_protected_line(
        carrier,
        Role::Initiator,
        &binding,
        policy(),
        descriptor.copy_protected_session_psk_for_attempt(),
        &mut entropy,
    )
    .unwrap();
    protected.close().unwrap();
    drop(protected);
    assert_eq!(socket.attachments, 1);
    assert_eq!(socket.envelopes, 1);
    assert_eq!(socket.closes, 1);
    assert_eq!(entropy.requests_used(), 1);
}
