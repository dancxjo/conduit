use super::*;
use alloc::{string::ToString, vec};

use crate::{
    RendezvousAuthentication, RendezvousCandidate, RendezvousLineFamily,
    RunningHostRendezvousDescriptor,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};

const KEY_ID: &[u8] = b"operator-signing-key-1";

struct TestPolicy {
    key: SigningKey,
}

impl TestPolicy {
    fn new() -> Self {
        Self {
            key: SigningKey::from_bytes(&[0x27; 32]),
        }
    }
}

impl RendezvousCoseSigner for TestPolicy {
    fn key_id(&self) -> &[u8] {
        KEY_ID
    }

    fn sign(
        &self,
        sig_structure: &[u8],
        signature: &mut [u8; RENDEZVOUS_COSE_SIGNATURE_BYTES],
    ) -> Result<(), RendezvousCosePolicyRefusal> {
        *signature = self.key.sign(sig_structure).to_bytes();
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
struct TestAttribution(&'static str);

impl RendezvousCoseVerifier for TestPolicy {
    type Attribution = TestAttribution;

    fn verify(
        &self,
        key_id: &[u8],
        sig_structure: &[u8],
        signature: &[u8; RENDEZVOUS_COSE_SIGNATURE_BYTES],
    ) -> Result<Self::Attribution, RendezvousCosePolicyRefusal> {
        if key_id != KEY_ID {
            return Err(RendezvousCosePolicyRefusal);
        }
        self.key
            .verifying_key()
            .verify(sig_structure, &Signature::from_bytes(signature))
            .map_err(|_| RendezvousCosePolicyRefusal)?;
        Ok(TestAttribution("configured test operator"))
    }
}

fn descriptor() -> RunningHostRendezvousDescriptor {
    RunningHostRendezvousDescriptor::new(
        vec![RendezvousCandidate {
            candidate_id: "candidate/relay".to_string(),
            line_family: RendezvousLineFamily::AuthenticatedConduitLine,
            reachability: "relay://operator.example/route/7".to_string(),
            authentication: RendezvousAuthentication {
                server_identity: "host/server/key-7".to_string(),
                transport_binding_sha256: [0x44; 32],
            },
            expires_at_millis: 1_800_000_000_000,
            maximum_attempts: 3,
            attempt_timeout_millis: 30_000,
        }],
        [0x55; 32],
        1_700_000_000_000,
    )
    .unwrap()
}

fn signed_vector() -> (TestPolicy, [u8; MAX_RENDEZVOUS_COSE_SIGN1_BYTES], usize) {
    let policy = TestPolicy::new();
    let mut encoded = [0; MAX_RENDEZVOUS_COSE_SIGN1_BYTES];
    let length =
        encode_running_host_rendezvous_cose_sign1(&descriptor(), &policy, &mut encoded).unwrap();
    (policy, encoded, length)
}

#[test]
fn sign1_round_trip_returns_only_policy_attribution_and_descriptor() {
    let (policy, encoded, length) = signed_vector();
    let verified =
        decode_running_host_rendezvous_cose_sign1(&encoded[..length], 1_700_000_000_000, &policy)
            .unwrap();

    assert_eq!(verified.key_id, KEY_ID);
    assert_eq!(
        verified.attribution,
        TestAttribution("configured test operator")
    );
    assert_eq!(verified.descriptor.candidates().len(), 1);
    assert_eq!(
        verified
            .descriptor
            .candidates()
            .next()
            .unwrap()
            .candidate_id,
        "candidate/relay"
    );
}

#[test]
fn sign1_is_deterministic_and_matches_checked_vector() {
    let (policy, encoded, length) = signed_vector();
    let mut second = [0; MAX_RENDEZVOUS_COSE_SIGN1_BYTES];
    let second_length =
        encode_running_host_rendezvous_cose_sign1(&descriptor(), &policy, &mut second).unwrap();
    assert_eq!(&encoded[..length], &second[..second_length]);
    assert_eq!(
        hex(&encoded[..length]),
        include_str!("../../schemas/running-host-rendezvous-sign1-v1.hex").trim()
    );
}

#[test]
fn altered_signature_and_unknown_key_policy_refuse() {
    let (policy, mut encoded, length) = signed_vector();
    encoded[length - 1] ^= 1;
    assert!(matches!(
        decode_running_host_rendezvous_cose_sign1(&encoded[..length], 1_700_000_000_000, &policy),
        Err(RendezvousCoseRefusal::VerificationRejected)
    ));

    struct RejectAll;
    impl RendezvousCoseVerifier for RejectAll {
        type Attribution = ();
        fn verify(
            &self,
            _: &[u8],
            _: &[u8],
            _: &[u8; RENDEZVOUS_COSE_SIGNATURE_BYTES],
        ) -> Result<(), RendezvousCosePolicyRefusal> {
            Err(RendezvousCosePolicyRefusal)
        }
    }
    let (_, encoded, length) = signed_vector();
    assert!(matches!(
        decode_running_host_rendezvous_cose_sign1(
            &encoded[..length],
            1_700_000_000_000,
            &RejectAll
        ),
        Err(RendezvousCoseRefusal::VerificationRejected)
    ));
}

#[test]
fn sign1_rejects_bounds_truncation_and_noncanonical_headers() {
    struct BadKeyId;
    impl RendezvousCoseSigner for BadKeyId {
        fn key_id(&self) -> &[u8] {
            &[]
        }
        fn sign(
            &self,
            _: &[u8],
            _: &mut [u8; RENDEZVOUS_COSE_SIGNATURE_BYTES],
        ) -> Result<(), RendezvousCosePolicyRefusal> {
            Ok(())
        }
    }
    assert_eq!(
        encode_running_host_rendezvous_cose_sign1(
            &descriptor(),
            &BadKeyId,
            &mut [0; MAX_RENDEZVOUS_COSE_SIGN1_BYTES]
        ),
        Err(RendezvousCoseRefusal::KeyIdBound)
    );

    let (policy, encoded, length) = signed_vector();
    assert!(matches!(
        decode_running_host_rendezvous_cose_sign1(
            &encoded[..length - 1],
            1_700_000_000_000,
            &policy
        ),
        Err(RendezvousCoseRefusal::Truncated)
    ));
    assert_eq!(
        decode_running_host_rendezvous_cose_sign1(
            &[0; MAX_RENDEZVOUS_COSE_SIGN1_BYTES + 1],
            1_700_000_000_000,
            &policy
        )
        .err(),
        Some(RendezvousCoseRefusal::EncodedBound)
    );
}

fn hex(bytes: &[u8]) -> alloc::string::String {
    use core::fmt::Write;
    let mut value = alloc::string::String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").unwrap();
    }
    value
}
