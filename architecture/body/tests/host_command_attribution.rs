#![cfg(feature = "authenticated-admission")]
use conduit_body::*;
use conduit_core::{HostAdvertisement, HostCommand, OfferGeneration, PROTOCOL_VERSION};
use ed25519_dalek::{Signer, SigningKey};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

struct Fixture {
    member: MembershipCredential,
    host: HostAdvertisement,
    issuer: FederationPeer,
    key: SigningKey,
}
impl Fixture {
    fn new() -> Self {
        let key = SigningKey::from_bytes(&[9; 32]);
        Self {
            member: serde_json::from_value(serde_json::json!({
                "credential_id":"credential/1","body_id":"body/1","part_id":"part/1",
                "host_id":"host/1","boot_id":"boot/1","issued_at_millis":1,
            }))
            .unwrap(),
            host: HostAdvertisement {
                declared_name: None,
                protocol_version: PROTOCOL_VERSION,
                host_id: "host/1".into(),
                boot_id: "boot/1".into(),
                offer_generation: OfferGeneration(1),
                profile: "profile/1".into(),
                bases: vec![],
                resources: vec![],
                capabilities: vec![],
                planner_capabilities: vec![],
            },
            issuer: FederationPeer {
                host_id: "issuer/1".into(),
                boot_id: "issuer/boot/1".into(),
                offer_generation: OfferGeneration(1),
                verifying_key: key.verifying_key().to_bytes(),
            },
            key,
        }
    }
    fn slot(&self, nonce: [u8; 32]) -> IssuedHostCommandChallenge<'_> {
        IssuedHostCommandChallenge::issue(
            &self.member,
            &self.host,
            &self.issuer,
            HostCommandReceiveBounds {
                payload_contract: "fixture/host-command-json@1",
                maximum_payload_bytes: 64,
                lifetime_millis: 10,
            },
            nonce,
            10,
        )
        .unwrap()
    }
}

#[test]
fn exact_core_command_is_attributed_without_allocation_or_copying_payload() {
    let f = Fixture::new();
    let payload = serde_json::to_vec(&HostCommand::Inspect).unwrap();
    let ((), cost) = probe::observe(|| {
        let slot = f.slot([1; 32]);
        let transcript = host_command_attribution_transcript(slot.challenge(), &payload).unwrap();
        let signature = f.key.sign(&transcript).to_bytes();
        let command = slot.authenticate(&payload, &signature, 11).unwrap();
        assert_eq!(command.payload(), payload);
        assert_eq!(command.payload().as_ptr(), payload.as_ptr());
        assert_eq!(command.challenge().body_id, f.member.body_id.as_str());
        assert_eq!(command.challenge().part_id, f.member.part_id.as_str());
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}

#[test]
fn all_context_fields_and_exact_payload_bytes_are_signed() {
    let f = Fixture::new();
    let payload = serde_json::to_vec(&HostCommand::Inspect).unwrap();
    for index in 0..17 {
        let slot = f.slot([1; 32]);
        let mut altered = *slot.challenge();
        match index {
            0 => altered.body_id = "body/other",
            1 => altered.part_id = "part/other",
            2 => altered.credential_id = "credential/other",
            3 => altered.receiver_host = "host/other",
            4 => altered.receiver_boot = "boot/other",
            5 => altered.receiver_profile = "profile/other",
            6 => altered.issuer_host = "issuer/other",
            7 => altered.issuer_boot = "issuer/boot/other",
            8 => altered.payload_contract = "fixture/another-command@1",
            9 => altered.receiver_generation += 1,
            10 => altered.issuer_generation += 1,
            11 => altered.maximum_payload_bytes += 1,
            12 => altered.nonce[0] ^= 1,
            13 => altered.issued_at_millis -= 1,
            14 => altered.expires_at_millis += 1,
            15 => altered.issuer_key[0] ^= 1,
            _ => {}
        }
        let signed_payload = if index == 16 {
            b"\"Inspect\" ".as_slice()
        } else {
            &payload
        };
        let signature = f
            .key
            .sign(&host_command_attribution_transcript(&altered, signed_payload).unwrap())
            .to_bytes();
        assert!(
            matches!(
                slot.authenticate(&payload, &signature, 11),
                Err(HostCommandAttributionRefusal::Signature)
            ),
            "field {index}"
        );
    }
}

#[test]
fn stale_nonce_expiry_wrong_key_and_invalid_extents_refuse() {
    let f = Fixture::new();
    let payload = b"\"Inspect\"";
    let signature = f
        .key
        .sign(&host_command_attribution_transcript(f.slot([1; 32]).challenge(), payload).unwrap())
        .to_bytes();
    assert!(matches!(
        f.slot([2; 32]).authenticate(payload, &signature, 11),
        Err(HostCommandAttributionRefusal::Signature)
    ));
    for at in [9, 20, u64::MAX] {
        assert!(matches!(
            f.slot([1; 32]).authenticate(payload, &signature, at),
            Err(HostCommandAttributionRefusal::Clock)
        ));
    }
    let other = SigningKey::from_bytes(&[8; 32]);
    let signature = other
        .sign(&host_command_attribution_transcript(f.slot([1; 32]).challenge(), payload).unwrap())
        .to_bytes();
    assert!(matches!(
        f.slot([1; 32]).authenticate(payload, &signature, 11),
        Err(HostCommandAttributionRefusal::Signature)
    ));
    for bytes in [b"".as_slice(), &[0; 65]] {
        assert!(matches!(
            f.slot([1; 32]).authenticate(bytes, &[0; 64], 11),
            Err(HostCommandAttributionRefusal::Bounds)
        ));
    }
    assert!(matches!(
        f.slot([1; 32]).authenticate(payload, &[0; 63], 11),
        Err(HostCommandAttributionRefusal::Signature)
    ));
}

#[test]
fn admission_receipt_proof_cannot_be_reused_as_a_host_command() {
    let f = Fixture::new();
    let payload = b"\"Inspect\"";
    let transcript = admission_receipt_attestation_transcript(
        payload,
        f.issuer.host_id.as_str(),
        f.issuer.boot_id.as_str(),
        f.issuer.offer_generation.0,
        &f.issuer.verifying_key,
        &[1; 32],
    )
    .unwrap();
    let signature = f.key.sign(&transcript).to_bytes();
    assert!(matches!(
        f.slot([1; 32]).authenticate(payload, &signature, 11),
        Err(HostCommandAttributionRefusal::Signature)
    ));
}

#[test]
fn local_context_and_bounds_must_be_current_and_finite() {
    let mut f = Fixture::new();
    for case in 0..9 {
        let mut bounds = HostCommandReceiveBounds {
            payload_contract: "fixture/command",
            maximum_payload_bytes: 64,
            lifetime_millis: 10,
        };
        let nonce = if case == 0 { [0; 32] } else { [1; 32] };
        let at = if case == 1 { u64::MAX } else { 10 };
        match case {
            2 => bounds.maximum_payload_bytes = 0,
            3 => bounds.lifetime_millis = 0,
            4 => bounds.payload_contract = "",
            5 => f.host.boot_id = "boot/stale".into(),
            6 => {
                f.host.boot_id = "boot/1".into();
                f.host.offer_generation = OfferGeneration(0);
            }
            7 => {
                f.host.offer_generation = OfferGeneration(1);
                f.issuer.offer_generation = OfferGeneration(0);
            }
            8 => {
                f.issuer.offer_generation = OfferGeneration(1);
                f.issuer.verifying_key = [0; 32];
            }
            _ => {}
        }
        assert!(
            IssuedHostCommandChallenge::issue(&f.member, &f.host, &f.issuer, bounds, nonce, at)
                .is_err(),
            "case {case}"
        );
    }
}
