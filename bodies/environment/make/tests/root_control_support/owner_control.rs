//! Exact production verifier with explicit CPU signing keys, not device trust.
use super::*;
use ed25519_dalek::{Signer, SigningKey};
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_owner_control.rs"]
mod control;
use control::*;
fn request() -> Request<'static> {
    Request {
        protocol: PROTOCOL,
        operation: Operation::Prepare,
        sequence: 1,
        host: "host/network",
        boot: "boot/current",
        offer_generation: 1,
        body: "body/fixture",
        wake: "wake/fixture",
        body_plan: "body-plan/fixture",
        body_play: "body-play/fixture",
        plan: "plan/fixture",
        fragment: "fragment/fixture",
        fragment_play: "fragment-play/fixture",
        assignment_seal: [7; 32],
    }
}
fn signature(key: &SigningKey, request: &Request<'_>) -> [u8; 64] {
    key.sign(&request.transcript().unwrap()).to_bytes()
}
#[test]
fn absent_or_weak_provisioning_cannot_be_replaced_by_request_metadata() {
    assert_eq!(MAXIMUM_REQUEST_BYTES, 2048);
    let mut disabled = OwnerControl::new(None).unwrap();
    assert!(!disabled.configured());
    assert!(matches!(
        disabled.authenticate(request(), &[0; 64], "host/network", "boot/current", 1),
        Err(Refusal::Disabled)
    ));
    assert!(matches!(
        OwnerControl::new(Some([0; 32])),
        Err(Refusal::InvalidKey)
    ));
    let mut identity = [0; 32];
    identity[0] = 1;
    assert!(matches!(
        OwnerControl::new(Some(identity)),
        Err(Refusal::InvalidKey)
    ));
}
#[test]
fn every_scoped_fact_and_operation_is_authenticated_before_sequence_consumption() {
    let key = SigningKey::from_bytes(&[9; 32]);
    let original_signature = signature(&key, &request());
    for change in 0..15 {
        let mut gate = OwnerControl::new(Some(key.verifying_key().to_bytes())).unwrap();
        assert!(gate.configured());
        let mut altered = request();
        match change {
            0 => altered.host = "foreign/host",
            1 => altered.boot = "stale/boot",
            2 => altered.offer_generation += 1,
            3 => altered.body = "other/body",
            4 => altered.wake = "other/wake",
            5 => altered.body_plan = "other/body-plan",
            6 => altered.body_play = "other/body-play",
            7 => altered.plan = "other/plan",
            8 => altered.fragment = "other/fragment",
            9 => altered.fragment_play = "other/play",
            10 => altered.assignment_seal[0] ^= 1,
            11 => altered.operation = Operation::Start,
            12 => altered.sequence += 1,
            13 => altered.protocol += 1,
            _ => altered.sequence = 0,
        }
        let (result, memory) = allocation_probe::observe(|| {
            gate.authenticate(
                altered,
                &original_signature,
                "host/network",
                "boot/current",
                1,
            )
        });
        assert!(result.is_err());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(gate.last_sequence(), 0);
        let (result, memory) = allocation_probe::observe(|| {
            gate.authenticate(
                request(),
                &original_signature,
                "host/network",
                "boot/current",
                1,
            )
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(result.unwrap().into_request().operation, Operation::Prepare);
        assert_eq!(gate.last_sequence(), 1);
        assert!(matches!(
            gate.authenticate(
                request(),
                &original_signature,
                "host/network",
                "boot/current",
                1
            ),
            Err(Refusal::Replay)
        ));
    }
}
#[test]
fn foreign_key_truncation_unknown_fields_and_ambiguous_ids_refuse() {
    let key = SigningKey::from_bytes(&[9; 32]);
    let foreign = SigningKey::from_bytes(&[8; 32]);
    let mut gate = OwnerControl::new(Some(key.verifying_key().to_bytes())).unwrap();
    assert!(matches!(
        gate.authenticate(
            request(),
            &signature(&foreign, &request()),
            "host/network",
            "boot/current",
            1
        ),
        Err(Refusal::Signature)
    ));
    for id in ["", "a\nb", "snowman☃"] {
        let mut bad = request();
        bad.body = id;
        assert_eq!(bad.transcript(), Err(Refusal::Malformed));
    }
    let long = "a".repeat(129);
    let mut bad = request();
    bad.plan = &long;
    assert_eq!(bad.transcript(), Err(Refusal::Malformed));
    let mut encoded = serde_json::to_value(request()).unwrap();
    encoded["started"] = true.into();
    let bytes = serde_json::to_vec(&encoded).unwrap();
    assert!(serde_json::from_slice::<Request<'_>>(&bytes).is_err());
    assert!(serde_json::from_slice::<Request<'_>>(&bytes[..bytes.len() - 1]).is_err());
    let mut second = request();
    second.sequence = u64::MAX;
    let sig = signature(&key, &second);
    gate.authenticate(second, &sig, "host/network", "boot/current", 1)
        .unwrap();
    assert_eq!(gate.last_sequence(), u64::MAX);
    assert!(matches!(
        gate.authenticate(
            request(),
            &signature(&key, &request()),
            "host/network",
            "boot/current",
            1
        ),
        Err(Refusal::Replay)
    ));
}
