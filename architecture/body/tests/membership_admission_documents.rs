//! CPU membership/document checks, not equipment admission or durable Root proof.
use conduit_body::*;
use conduit_core as c;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};
fn fixture() -> (
    AdmissionManager,
    BodyMembership,
    PortableMembershipAdmissionRequest,
) {
    let body = Body::born(
        "source/member".into(),
        "checked/member".into(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let membership = BodyMembership::new(body.body_id).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([21; 32]).unwrap(),
            [22; 32],
            100,
            10000,
        )
        .unwrap();
    let host = c::HostId::from("host/worker");
    let boot = c::BootId::from("boot/worker");
    let generation = c::OfferGeneration(3);
    let signature = invitation
        .secret
        .sign(&invitation.signing_transcript(&host, &boot, generation));
    let request = PortableMembershipAdmissionRequest {
        schema: MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
        protocol_version: c::PROTOCOL_VERSION,
        invitation_id: invitation.invitation_id,
        body_id: invitation.body_id,
        host_id: host,
        boot_id: boot,
        offer_generation: generation,
        nonce: invitation.nonce,
        signature,
    };
    (manager, membership, request)
}
fn receipt() -> (
    PortableMembershipAdmissionRequest,
    PortableMembershipAdmissionReceipt,
    SigningKey,
) {
    let (mut manager, mut membership, request) = fixture();
    let credential = manager
        .complete_spawn_for_host(
            &mut membership,
            request.host_scope(),
            &request.admission_proof().unwrap(),
            101,
            AdmissionSigns {
                part_admitted: "sign/part".into(),
                host_attached: "sign/host".into(),
                candidate_admitted: "sign/candidate".into(),
            },
        )
        .unwrap();
    let key = SigningKey::from_bytes(&[31; 32]);
    let mut receipt = PortableMembershipAdmissionReceipt {
        schema: MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA.into(),
        protocol_version: c::PROTOCOL_VERSION,
        credential,
        membership_revision: membership.revision.0,
        request_digest: request.digest(),
        producer_host: "host/root".into(),
        producer_boot: "boot/root".into(),
        producer_key: key.verifying_key().to_bytes(),
        invitation_id: request.invitation_id.clone(),
        offer_generation: request.offer_generation,
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
        signature: [0; 64],
    };
    receipt.signature = key.sign(&receipt.transcript()).to_bytes();
    (request, receipt, key)
}
#[test]
fn distinct_request_roundtrips_and_reuses_real_invitation_completion_without_equipment() {
    let (mut manager, mut membership, request) = fixture();
    let bytes = serde_json::to_vec(&request).unwrap();
    let decoded = PortableMembershipAdmissionRequest::decode(&bytes).unwrap();
    assert_eq!(decoded, request);
    let wire: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(wire.get("host_advertisement").is_none());
    assert!(wire.get("membership_admitted").is_none());
    assert_eq!(wire["signature"].as_array().unwrap().len(), 64);
    let proof = decoded.admission_proof().unwrap();
    let credential = manager
        .complete_spawn_for_host(
            &mut membership,
            decoded.host_scope(),
            &proof,
            101,
            AdmissionSigns {
                part_admitted: "sign/part".into(),
                host_attached: "sign/host".into(),
                candidate_admitted: "sign/candidate".into(),
            },
        )
        .unwrap();
    assert_eq!(credential.host_id, request.host_id);
    assert_eq!(membership.parts.len(), 1);
    assert_eq!(
        manager.complete_spawn_for_host(
            &mut membership,
            decoded.host_scope(),
            &proof,
            102,
            AdmissionSigns {
                part_admitted: "sign/replay-part".into(),
                host_attached: "sign/replay-host".into(),
                candidate_admitted: "sign/replay-candidate".into()
            }
        ),
        Err(AdmissionRefusal::Replay)
    );
}
#[test]
fn bounded_decode_refuses_large_frames_before_parsing_and_strict_unknown_fields() {
    let (_, _, request) = fixture();
    let mut wire = serde_json::to_value(request).unwrap();
    for field in [
        "host_advertisement",
        "capabilities",
        "membership_admitted",
        "current_offers_available",
        "plan_created",
        "play_created",
    ] {
        wire[field] = json!([]);
        assert!(
            PortableMembershipAdmissionRequest::decode(&serde_json::to_vec(&wire).unwrap())
                .is_err()
        );
        wire.as_object_mut().unwrap().remove(field);
    }
    for huge in [
        vec![b' '; MAX_MEMBERSHIP_ADMISSION_REQUEST_BYTES + 1],
        format!("{{\"host_id\":\"{}\"}}", "x".repeat(100000)).into_bytes(),
        format!("{{\"signature\":[{}]}}", "0,".repeat(50000)).into_bytes(),
    ] {
        assert_eq!(
            PortableMembershipAdmissionRequest::decode(&huge),
            Err(MembershipAdmissionDocumentRefusal::Capacity)
        );
    }
    assert_eq!(
        PortableMembershipAdmissionRequest::decode(&[]),
        Err(MembershipAdmissionDocumentRefusal::Capacity)
    );
}
#[test]
fn request_refuses_wrong_schema_protocol_generation_identity_nonce_and_signature_extent() {
    let (_, _, request) = fixture();
    let valid = serde_json::to_value(request).unwrap();
    for (field, value) in [
        ("schema", json!("conduit.body/spawn-admission-request@1")),
        ("protocol_version", json!(c::PROTOCOL_VERSION + 1)),
        ("offer_generation", json!(0)),
        ("host_id", json!("")),
        ("boot_id", json!("x".repeat(257))),
        ("invitation_id", json!("")),
        ("body_id", json!("")),
        ("nonce", json!(vec![0; 32])),
        ("signature", json!(vec![1; 63])),
        ("signature", json!(vec![1; 65])),
        ("signature", json!(vec![256; 64])),
        ("signature", json!("00".repeat(64))),
    ] {
        let mut bad = valid.clone();
        bad[field] = value;
        assert!(
            PortableMembershipAdmissionRequest::decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "accepted {field}"
        );
    }
    // Well-framed proof bytes are not authenticated by document decoding.
    let mut bad = valid;
    bad["signature"] = json!(vec![0; 64]);
    let decoded =
        PortableMembershipAdmissionRequest::decode(&serde_json::to_vec(&bad).unwrap()).unwrap();
    let (mut manager, mut membership, _) = fixture();
    assert_eq!(
        manager.complete_spawn_for_host(
            &mut membership,
            decoded.host_scope(),
            &decoded.admission_proof().unwrap(),
            101,
            AdmissionSigns {
                part_admitted: "sign/part".into(),
                host_attached: "sign/host".into(),
                candidate_admitted: "sign/candidate".into()
            }
        ),
        Err(AdmissionRefusal::InvalidProof)
    );
    assert!(membership.parts.is_empty());
}
#[test]
fn receipt_verification_requires_independent_root_pin_and_exact_request() {
    let (request, receipt, key) = receipt();
    let encoded = serde_json::to_vec(&receipt).unwrap();
    assert!(encoded.len() <= MAX_MEMBERSHIP_ADMISSION_RECEIPT_BYTES);
    let decoded = PortableMembershipAdmissionReceipt::decode(&encoded).unwrap();
    assert_eq!(decoded, receipt);
    decoded
        .verify(&key.verifying_key().to_bytes(), &request)
        .unwrap();
    assert_eq!(
        decoded.verify(
            &SigningKey::from_bytes(&[32; 32]).verifying_key().to_bytes(),
            &request
        ),
        Err(MembershipAdmissionDocumentRefusal::RootKey)
    );
    let mut foreign = request.clone();
    foreign.boot_id = "boot/other".into();
    assert_eq!(
        decoded.validate_against(&foreign),
        Err(MembershipAdmissionDocumentRefusal::ReceiptBasis)
    );
    let mut bad = decoded.clone();
    bad.signature[0] ^= 1;
    assert_eq!(
        bad.verify(&key.verifying_key().to_bytes(), &request),
        Err(MembershipAdmissionDocumentRefusal::Signature)
    );
    assert_eq!(
        PortableMembershipAdmissionReceipt::decode(&vec![
            b' ';
            MAX_MEMBERSHIP_ADMISSION_RECEIPT_BYTES + 1
        ]),
        Err(MembershipAdmissionDocumentRefusal::Capacity)
    );
}
#[test]
fn signed_receipt_cannot_change_membership_scope_or_assert_offer_plan_play() {
    let (request, receipt, key) = receipt();
    let valid = serde_json::to_value(&receipt).unwrap();
    for (field, value) in [
        ("schema", json!("conduit.environment/joined-host@1")),
        ("protocol_version", json!(c::PROTOCOL_VERSION + 1)),
        ("membership_revision", json!(0)),
        ("producer_host", json!("")),
        ("producer_boot", json!("")),
        ("membership_admitted", json!(false)),
        ("current_offers_available", json!(true)),
        ("plan_created", json!(true)),
        ("play_created", json!(true)),
        ("signature", json!(vec![0; 65])),
    ] {
        let mut bad = valid.clone();
        bad[field] = value;
        assert!(
            PortableMembershipAdmissionReceipt::decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "accepted {field}"
        );
    }
    for field in [
        "membership_revision",
        "producer_host",
        "producer_boot",
        "offer_generation",
    ] {
        let mut changed = valid.clone();
        changed[field] = match field {
            "membership_revision" => json!(receipt.membership_revision + 1),
            "offer_generation" => json!(receipt.offer_generation.0 + 1),
            _ => json!("different"),
        };
        let changed =
            PortableMembershipAdmissionReceipt::decode(&serde_json::to_vec(&changed).unwrap())
                .unwrap();
        assert_ne!(changed.transcript(), receipt.transcript());
        assert!(changed
            .verify(&key.verifying_key().to_bytes(), &request)
            .is_err());
    }
    let mut changed = valid;
    changed["host_advertisement"] = json!({});
    assert!(
        PortableMembershipAdmissionReceipt::decode(&serde_json::to_vec(&changed).unwrap()).is_err()
    );
}

#[test]
fn receipt_correlates_exact_request_proof_bytes_and_authenticates_its_digest() {
    let (request, receipt, key) = receipt();
    for mutation in 0..7 {
        let mut changed = request.clone();
        match mutation {
            0 => changed.nonce[0] ^= 1,
            1 => changed.signature[0] ^= 1,
            2 => changed.protocol_version += 1,
            3 => changed.host_id = "host/other".into(),
            4 => changed.boot_id = "boot/other".into(),
            5 => changed.invitation_id = serde_json::from_str("\"different-invitation\"").unwrap(),
            6 => changed.body_id = serde_json::from_str("\"different-body\"").unwrap(),
            _ => unreachable!(),
        }
        assert_ne!(changed.digest(), request.digest());
        assert!(receipt.validate_against(&changed).is_err());
        assert!(receipt
            .verify(&key.verifying_key().to_bytes(), &changed)
            .is_err());
    }
    let mut changed = receipt.clone();
    changed.request_digest[0] ^= 1;
    assert_ne!(changed.transcript(), receipt.transcript());
    assert_eq!(
        changed.validate_against(&request),
        Err(MembershipAdmissionDocumentRefusal::ReceiptBasis)
    );
    // Preserve correlation with a different request while retaining the original
    // signature: only the independently pinned Root may endorse that change.
    let mut changed_request = request.clone();
    changed_request.signature[0] ^= 1;
    changed.request_digest = changed_request.digest();
    changed.validate_against(&changed_request).unwrap();
    assert_eq!(
        changed.verify(&key.verifying_key().to_bytes(), &changed_request),
        Err(MembershipAdmissionDocumentRefusal::Signature)
    );
}
