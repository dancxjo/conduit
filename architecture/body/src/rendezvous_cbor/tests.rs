use super::*;
use alloc::{format, string::ToString, vec, vec::Vec};

use crate::{RendezvousAuthentication, RENDEZVOUS_DESCRIPTOR_PROTOCOL};

fn candidate(id: &str, family: RendezvousLineFamily) -> RendezvousCandidate {
    RendezvousCandidate {
        candidate_id: id.to_string(),
        line_family: family,
        reachability: match family {
            RendezvousLineFamily::LocalLoopbackWebSocket => {
                "ws://127.0.0.1:4173/conduit".to_string()
            }
            RendezvousLineFamily::WebRtcDataChannel => {
                "webrtc-bootstrap:operator/negotiation-7".to_string()
            }
            _ => "relay://operator.example/route/7".to_string(),
        },
        authentication: RendezvousAuthentication {
            server_identity: "host/server/key-7".to_string(),
            transport_binding_sha256: [0x44; 32],
        },
        expires_at_millis: 1_800_000_000_000,
        maximum_attempts: 3,
        attempt_timeout_millis: 30_000,
    }
}

fn descriptor() -> RunningHostRendezvousDescriptor {
    RunningHostRendezvousDescriptor::new(
        vec![
            candidate(
                "candidate/direct",
                RendezvousLineFamily::AuthenticatedTlsStream,
            ),
            candidate(
                "candidate/relay",
                RendezvousLineFamily::AuthenticatedConduitLine,
            ),
        ],
        [0x55; 32],
        1_700_000_000_000,
    )
    .unwrap()
}

fn canonical_vector() -> Vec<u8> {
    let mut output = [0; MAX_RENDEZVOUS_CBOR_BYTES];
    let length = encode_running_host_rendezvous_cbor(&descriptor(), &mut output).unwrap();
    let encoded = output[..length].to_vec();
    assert_eq!(encoded, checked_fixture());
    encoded
}

fn checked_fixture() -> Vec<u8> {
    include_str!("../../schemas/running-host-rendezvous-v1.hex")
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
fn owned_and_borrowed_profiles_round_trip_without_semantic_loss_or_text_copies() {
    let source = descriptor();
    let encoded = canonical_vector();
    let decoded = decode_running_host_rendezvous_cbor(&encoded, 1_700_000_000_000).unwrap();
    let actual: Vec<_> = decoded.candidates().copied().collect();
    assert_eq!(actual.len(), source.candidates.len());
    for (actual, expected) in actual.iter().zip(&source.candidates) {
        assert_eq!(actual.candidate_id, expected.candidate_id);
        assert_eq!(actual.line_family, expected.line_family);
        assert_eq!(actual.reachability, expected.reachability);
        assert_eq!(
            actual.server_identity,
            expected.authentication.server_identity
        );
        assert_eq!(
            actual.transport_binding_sha256,
            expected.authentication.transport_binding_sha256
        );
        assert_eq!(actual.expires_at_millis, expected.expires_at_millis);
        assert_eq!(actual.maximum_attempts, expected.maximum_attempts);
        assert_eq!(
            actual.attempt_timeout_millis,
            expected.attempt_timeout_millis
        );
        let start = encoded.as_ptr() as usize;
        let end = start + encoded.len();
        assert!((start..end).contains(&(actual.candidate_id.as_ptr() as usize)));
        assert!((start..end).contains(&(actual.reachability.as_ptr() as usize)));
        assert!((start..end).contains(&(actual.server_identity.as_ptr() as usize)));
    }
    assert_eq!(decoded.copy_session_secret_for_attempt(), [0x55; 32]);
    assert!(format!("{decoded:?}").contains("[REDACTED]"));

    let mut second = [0; MAX_RENDEZVOUS_CBOR_BYTES];
    let second_len = encode_running_host_rendezvous_cbor(&source, &mut second).unwrap();
    assert_eq!(&second[..second_len], encoded);
}

#[test]
fn webrtc_family_round_trips_through_the_shared_wire_profile() {
    let source = RunningHostRendezvousDescriptor::new(
        vec![candidate(
            "candidate/webrtc",
            RendezvousLineFamily::WebRtcDataChannel,
        )],
        [0x66; 32],
        1_700_000_000_000,
    )
    .unwrap();
    let mut encoded = [0; MAX_RENDEZVOUS_CBOR_BYTES];
    let length = encode_running_host_rendezvous_cbor(&source, &mut encoded).unwrap();
    let decoded =
        decode_running_host_rendezvous_cbor(&encoded[..length], 1_700_000_000_000).unwrap();
    assert_eq!(
        decoded.candidates().next().unwrap().line_family,
        RendezvousLineFamily::WebRtcDataChannel
    );
}

#[test]
fn malformed_stale_version_family_and_noncanonical_inputs_refuse_distinctly() {
    let canonical = canonical_vector();

    let mut truncated = canonical.clone();
    truncated.pop();
    assert_eq!(
        decode_running_host_rendezvous_cbor(&truncated, 1_700_000_000_000).unwrap_err(),
        RendezvousCborRefusal::Truncated
    );
    assert_eq!(
        decode_running_host_rendezvous_cbor(&canonical, 1_900_000_000_000).unwrap_err(),
        RendezvousCborRefusal::Descriptor(RendezvousDescriptorRefusal::Expired)
    );

    let mut wrong_version = canonical.clone();
    assert_eq!(&wrong_version[..3], &[0xa3, 0, 1]);
    wrong_version[2] = 2;
    assert_eq!(
        decode_running_host_rendezvous_cbor(&wrong_version, 1_700_000_000_000).unwrap_err(),
        RendezvousCborRefusal::UnsupportedVersion
    );

    let family_offset = first_family_offset(&canonical);
    let mut wrong_family = canonical.clone();
    wrong_family[family_offset] = 9;
    assert_eq!(
        decode_running_host_rendezvous_cbor(&wrong_family, 1_700_000_000_000).unwrap_err(),
        RendezvousCborRefusal::UnsupportedLineFamily
    );

    let mut noncanonical = canonical.clone();
    noncanonical.insert(2, 0x18);
    assert_eq!(
        decode_running_host_rendezvous_cbor(&noncanonical, 1_700_000_000_000).unwrap_err(),
        RendezvousCborRefusal::NonCanonical
    );
}

#[test]
fn mandatory_duplicate_extension_and_encoded_bounds_are_explicit() {
    let canonical = canonical_vector();

    let mut duplicate = canonical.clone();
    duplicate[0] = 0xa4;
    duplicate.extend_from_slice(&[0, 1]);
    assert_eq!(
        decode_running_host_rendezvous_cbor(&duplicate, 1_700_000_000_000).unwrap_err(),
        RendezvousCborRefusal::DuplicateField
    );

    let mut mandatory = canonical.clone();
    mandatory[0] = 0xa4;
    mandatory.extend_from_slice(&[3, 0x40]);
    assert_eq!(
        decode_running_host_rendezvous_cbor(&mandatory, 1_700_000_000_000).unwrap_err(),
        RendezvousCborRefusal::UnsupportedMandatoryField
    );

    let mut extended = canonical.clone();
    extended[0] = 0xa4;
    extended.extend_from_slice(&[0x18, 0x80, 0x42, 1, 2]);
    let decoded = decode_running_host_rendezvous_cbor(&extended, 1_700_000_000_000).unwrap();
    assert_eq!(
        decoded.extensions().copied().collect::<Vec<_>>(),
        vec![BorrowedRendezvousExtension {
            key: 128,
            value: &[1, 2]
        }]
    );

    let oversized = [0; MAX_RENDEZVOUS_CBOR_BYTES + 1];
    assert_eq!(
        decode_running_host_rendezvous_cbor(&oversized, 0).unwrap_err(),
        RendezvousCborRefusal::EncodedBound
    );
}

#[test]
fn declared_maximum_is_the_exact_largest_accepted_canonical_shape() {
    let text_a = [b'a'; crate::MAX_RENDEZVOUS_TEXT_BYTES];
    let text_b = [b'b'; crate::MAX_RENDEZVOUS_TEXT_BYTES];
    let text_c = [b'c'; crate::MAX_RENDEZVOUS_TEXT_BYTES];
    let text_d = [b'd'; crate::MAX_RENDEZVOUS_TEXT_BYTES];
    let text = core::str::from_utf8(&text_a).unwrap();
    let make_candidate = |candidate_id| BorrowedRendezvousCandidate {
        candidate_id,
        line_family: RendezvousLineFamily::AuthenticatedConduitLine,
        reachability: text,
        server_identity: text,
        transport_binding_sha256: [1; 32],
        expires_at_millis: u64::MAX,
        maximum_attempts: crate::MAX_RENDEZVOUS_ATTEMPTS_PER_CANDIDATE,
        attempt_timeout_millis: crate::MAX_RENDEZVOUS_ATTEMPT_MILLIS,
    };
    let candidates = [
        make_candidate(core::str::from_utf8(&text_a).unwrap()),
        make_candidate(core::str::from_utf8(&text_b).unwrap()),
        make_candidate(core::str::from_utf8(&text_c).unwrap()),
        make_candidate(core::str::from_utf8(&text_d).unwrap()),
    ];
    let extension_bytes = [2; MAX_RENDEZVOUS_EXTENSION_BYTES];
    let extensions = [128, 129, 130, 131].map(|key| BorrowedRendezvousExtension {
        key,
        value: &extension_bytes,
    });
    let mut encoded = [0; MAX_RENDEZVOUS_CBOR_BYTES];
    assert_eq!(
        encode_fields(
            candidates.into_iter(),
            &[3; 32],
            extensions.into_iter(),
            &mut encoded,
        ),
        Ok(MAX_RENDEZVOUS_CBOR_BYTES)
    );
    assert!(decode_running_host_rendezvous_cbor(&encoded, 0).is_ok());
}

fn first_family_offset(input: &[u8]) -> usize {
    let mut decoder = Decoder::new(input);
    assert_eq!(decoder.map().unwrap(), Some(3));
    assert_eq!(decoder.u8().unwrap(), KEY_VERSION);
    assert_eq!(decoder.u8().unwrap(), WIRE_VERSION);
    assert_eq!(decoder.u8().unwrap(), KEY_CANDIDATES);
    assert_eq!(decoder.array().unwrap(), Some(2));
    assert_eq!(decoder.map().unwrap(), Some(7));
    assert_eq!(decoder.u8().unwrap(), 0);
    decoder.str().unwrap();
    assert_eq!(decoder.u8().unwrap(), 1);
    decoder.position()
}

#[test]
fn semantic_schema_remains_the_existing_running_host_contract() {
    assert_eq!(descriptor().schema, "conduit.host/rendezvous-descriptor@1");
    assert_eq!(RENDEZVOUS_DESCRIPTOR_PROTOCOL, 1);
    assert_ne!(RENDEZVOUS_CBOR_PROFILE, descriptor().schema);
}
