use conduit_body::{
    decode_running_host_rendezvous_cbor, encode_running_host_rendezvous_cbor,
    RendezvousAuthentication, RendezvousCandidate, RendezvousLineFamily,
    RunningHostRendezvousDescriptor, MAX_RENDEZVOUS_CBOR_BYTES,
};

fn fixture() -> Vec<u8> {
    include_str!("../../../architecture/body/schemas/running-host-rendezvous-v1.hex")
        .trim()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|digits| {
            let digit = |value| match value {
                b'0'..=b'9' => value - b'0',
                b'a'..=b'f' => value - b'a' + 10,
                _ => panic!("fixture is lowercase hexadecimal"),
            };
            (digit(digits[0]) << 4) | digit(digits[1])
        })
        .collect()
}

#[test]
fn std_host_consumes_the_checked_target_neutral_descriptor() {
    let fixture = fixture();
    let descriptor = decode_running_host_rendezvous_cbor(&fixture, 1_700_000_000_000).unwrap();
    let families: Vec<_> = descriptor
        .candidates()
        .map(|candidate| candidate.line_family)
        .collect();
    assert_eq!(
        families,
        vec![
            RendezvousLineFamily::AuthenticatedTlsStream,
            RendezvousLineFamily::AuthenticatedConduitLine,
        ]
    );
    assert_eq!(descriptor.copy_session_secret_for_attempt(), [0x55; 32]);
}

#[test]
fn std_host_consumes_the_shared_webrtc_candidate_semantics() {
    let descriptor = RunningHostRendezvousDescriptor::new(
        vec![RendezvousCandidate {
            candidate_id: "candidate/webrtc".into(),
            line_family: RendezvousLineFamily::WebRtcDataChannel,
            reachability: "webrtc-bootstrap:operator/negotiation-7".into(),
            authentication: RendezvousAuthentication {
                server_identity: "host/peer/key-7".into(),
                transport_binding_sha256: [0xef; 32],
            },
            expires_at_millis: 1_800_000_000_000,
            maximum_attempts: 1,
            attempt_timeout_millis: 10_000,
        }],
        [0xcd; 32],
        1_700_000_000_000,
    )
    .unwrap();
    let mut encoded = [0; MAX_RENDEZVOUS_CBOR_BYTES];
    let length = encode_running_host_rendezvous_cbor(&descriptor, &mut encoded).unwrap();
    let decoded =
        decode_running_host_rendezvous_cbor(&encoded[..length], 1_700_000_000_000).unwrap();
    let candidate = decoded.candidates().next().unwrap();
    assert_eq!(
        candidate.line_family,
        RendezvousLineFamily::WebRtcDataChannel
    );
    assert_eq!(
        candidate.reachability,
        "webrtc-bootstrap:operator/negotiation-7"
    );
}
