//! ConduitOS consumption of the shared target-neutral rendezvous bytes.

use conduit_body::{
    BorrowedRunningHostRendezvousDescriptor, RendezvousCborRefusal, RendezvousCoseRefusal,
    RendezvousCoseVerifier, RendezvousLineFamily, decode_running_host_rendezvous_cbor,
    decode_running_host_rendezvous_cose_sign1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendezvousDescriptorReceipt {
    pub candidate_count: u8,
    pub line_family_mask: u8,
}

/// Inspect an admitted caller-owned buffer without allocation or retaining its
/// capability. Actual Line attachment must independently admit a matching Base.
pub fn inspect(
    encoded: &[u8],
    now_millis: u64,
) -> Result<RendezvousDescriptorReceipt, RendezvousCborRefusal> {
    let descriptor = decode_running_host_rendezvous_cbor(encoded, now_millis)?;
    Ok(receipt(&descriptor))
}

/// Verify one signed interchange envelope under caller-owned policy. The
/// returned attribution remains separate from descriptor and Line admission.
pub fn inspect_signed<V: RendezvousCoseVerifier>(
    encoded: &[u8],
    now_millis: u64,
    verifier: &V,
) -> Result<(RendezvousDescriptorReceipt, V::Attribution), RendezvousCoseRefusal> {
    let verified = decode_running_host_rendezvous_cose_sign1(encoded, now_millis, verifier)?;
    Ok((receipt(&verified.descriptor), verified.attribution))
}

fn receipt(
    descriptor: &BorrowedRunningHostRendezvousDescriptor<'_>,
) -> RendezvousDescriptorReceipt {
    let mut line_family_mask = 0;
    for candidate in descriptor.candidates() {
        line_family_mask |= 1 << family_code(candidate.line_family);
    }
    RendezvousDescriptorReceipt {
        candidate_count: descriptor.candidates().len() as u8,
        line_family_mask,
    }
}

const fn family_code(family: RendezvousLineFamily) -> u8 {
    match family {
        RendezvousLineFamily::AuthenticatedTlsStream => 0,
        RendezvousLineFamily::AuthenticatedConduitLine => 1,
        RendezvousLineFamily::LocalLoopbackWebSocket => 2,
        RendezvousLineFamily::AttendedSerial => 3,
        RendezvousLineFamily::WebRtcDataChannel => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use conduit_body::{
        MAX_RENDEZVOUS_CBOR_BYTES, RENDEZVOUS_COSE_SIGNATURE_BYTES, RendezvousAuthentication,
        RendezvousCandidate, RendezvousCosePolicyRefusal, RunningHostRendezvousDescriptor,
        encode_running_host_rendezvous_cbor,
    };
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

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

    fn fixture_named(name: &str) -> Vec<u8> {
        let source = match name {
            "sign1" => include_str!(
                "../../../architecture/body/schemas/running-host-rendezvous-sign1-v1.hex"
            ),
            _ => panic!("unknown fixture"),
        };
        source
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

    struct TestPolicy;

    impl RendezvousCoseVerifier for TestPolicy {
        type Attribution = &'static str;

        fn verify(
            &self,
            key_id: &[u8],
            sig_structure: &[u8],
            signature: &[u8; RENDEZVOUS_COSE_SIGNATURE_BYTES],
        ) -> Result<Self::Attribution, RendezvousCosePolicyRefusal> {
            if key_id != b"operator-signing-key-1" {
                return Err(RendezvousCosePolicyRefusal);
            }
            let public_key = VerifyingKey::from_bytes(&[
                0xee, 0x45, 0xec, 0xb9, 0xac, 0xa0, 0x1a, 0x0a, 0xbd, 0x83, 0xef, 0x56, 0xdd, 0x98,
                0x5c, 0x8c, 0x87, 0x4e, 0x6e, 0x7f, 0x4a, 0xeb, 0xce, 0xdf, 0x20, 0xbd, 0x8d, 0x88,
                0xc2, 0xa0, 0xad, 0xd7,
            ])
            .map_err(|_| RendezvousCosePolicyRefusal)?;
            public_key
                .verify(sig_structure, &Signature::from_bytes(signature))
                .map_err(|_| RendezvousCosePolicyRefusal)?;
            Ok("configured test operator")
        }
    }

    #[test]
    fn conduitos_consumes_the_same_checked_descriptor_without_claiming_a_line() {
        assert_eq!(
            inspect(&fixture(), 1_700_000_000_000),
            Ok(RendezvousDescriptorReceipt {
                candidate_count: 2,
                line_family_mask: 0b0011,
            })
        );
    }

    #[test]
    fn conduitos_verifies_the_same_sign1_vector_without_granting_authority() {
        let (receipt, attribution) =
            inspect_signed(&fixture_named("sign1"), 1_700_000_000_000, &TestPolicy).unwrap();
        assert_eq!(receipt.candidate_count, 1);
        assert_eq!(receipt.line_family_mask, 0b0010);
        assert_eq!(attribution, "configured test operator");
    }

    #[test]
    fn conduitos_inspects_webrtc_but_does_not_claim_an_implementation() {
        let descriptor = RunningHostRendezvousDescriptor::new(
            alloc::vec![RendezvousCandidate {
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
        assert_eq!(
            inspect(&encoded[..length], 1_700_000_000_000),
            Ok(RendezvousDescriptorReceipt {
                candidate_count: 1,
                line_family_mask: 0b1_0000,
            })
        );
    }
}
