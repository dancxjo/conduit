use super::*;

use alloc::{string::ToString, vec};
use conduit_body::{
    RendezvousAuthentication, RendezvousCandidate, RunningHostRendezvousDescriptor,
    encode_running_host_rendezvous_cbor,
};
use sha2::{Digest, Sha256};

struct EchoWebSocket {
    sent: [u8; 64],
    sent_len: usize,
}

impl BinaryWebSocketIo for EchoWebSocket {
    fn send_binary(&mut self, payload: &[u8]) -> Result<(), WebSocketError> {
        if payload.len() > self.sent.len() {
            return Err(WebSocketError::RequestTooLarge);
        }
        self.sent[..payload.len()].copy_from_slice(payload);
        self.sent_len = payload.len();
        Ok(())
    }

    fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, WebSocketError> {
        if output.len() < self.sent_len {
            return Err(WebSocketError::ResponseTooLarge);
        }
        output[..self.sent_len].copy_from_slice(&self.sent[..self.sent_len]);
        Ok(self.sent_len)
    }
}

fn encoded_descriptor(certificate: &[u8]) -> alloc::vec::Vec<u8> {
    let descriptor = RunningHostRendezvousDescriptor::new(
        vec![RendezvousCandidate {
            candidate_id: "candidate/secure-lan".to_string(),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: "wss://lan.example:8443/conduit".to_string(),
            authentication: RendezvousAuthentication {
                server_identity: "lan.example".to_string(),
                transport_binding_sha256: Sha256::digest(certificate).into(),
            },
            expires_at_millis: 10_000,
            maximum_attempts: 1,
            attempt_timeout_millis: 2_000,
        }],
        [0x71; 32],
        1_000,
    )
    .unwrap();
    let mut encoded = vec![0; conduit_body::MAX_RENDEZVOUS_CBOR_BYTES];
    let length = encode_running_host_rendezvous_cbor(&descriptor, &mut encoded).unwrap();
    encoded.truncate(length);
    encoded
}

#[test]
fn canonical_descriptor_selects_one_exact_direct_tls_candidate() {
    let certificate = b"lan certificate";
    let encoded = encoded_descriptor(certificate);
    let descriptor = decode_running_host_rendezvous_cbor(&encoded, 1_000).unwrap();
    let candidate = descriptor
        .candidates()
        .find(|candidate| candidate.candidate_id == "candidate/secure-lan")
        .unwrap();
    assert_eq!(
        candidate.line_family,
        RendezvousLineFamily::AuthenticatedTlsStream
    );
    assert!(locator_matches(
        candidate.reachability,
        candidate.server_identity,
        8443
    ));
    assert!(certificate_matches(
        candidate.transport_binding_sha256,
        certificate
    ));
    assert_eq!(descriptor.copy_session_secret_for_attempt(), [0x71; 32]);
}

#[test]
fn direct_tls_line_is_bounded_and_closes_without_leaking_the_websocket() {
    let mut websocket = EchoWebSocket {
        sent: [0; 64],
        sent_len: 0,
    };
    let mut line = TlsRendezvousLine {
        websocket: &mut websocket,
        closed: false,
    };
    line.send(b"ordinary rendezvous frame").unwrap();
    let mut output = [0; 64];
    let length = line.receive(&mut output).unwrap();
    assert_eq!(&output[..length], b"ordinary rendezvous frame");
    assert_eq!(
        line.send(&[0; MAXIMUM_BINARY_MESSAGE_BYTES + 1]),
        Err(ConduitOsRendezvousLineError::Pressure)
    );
    line.close().unwrap();
    assert_eq!(
        line.receive(&mut output),
        Err(ConduitOsRendezvousLineError::Closed)
    );
}
