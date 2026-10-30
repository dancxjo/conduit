#![cfg(feature = "authenticated-admission")]

use conduit_body::{
    AdmissionDocumentRefusal as Refusal, AdmissionManager, AdmissionRefusal, AdmissionSigns, Body,
    BodyMembership, PortableAdmissionReceipt, PortableInvitation, PortableSpawnAdmissionRequest,
    RendezvousAuthentication, RendezvousCandidate, RendezvousLineFamily, RoutedAdmissionRequest,
    RoutedAdmissionResponse, SpawnInvitationSecret, SpawnRendezvousDescriptor,
    ADMISSION_SIGNATURE_BYTES, COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA,
    COMPACT_ROUTED_ADMISSION_RESPONSE_SCHEMA, INVITATION_SCHEMA,
    LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA, LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA,
    RENDEZVOUS_DESCRIPTOR_PROTOCOL, ROUTED_ADMISSION_REQUEST_SCHEMA,
    ROUTED_ADMISSION_RESPONSE_SCHEMA, ROUTED_INVITATION_SCHEMA, SPAWN_ADMISSION_RECEIPT_SCHEMA,
    SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::{HostAdvertisement, OfferGeneration, SignId, PROTOCOL_VERSION};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

const NOW: u64 = 10_000;
const EXPIRES: u64 = 20_000;

struct Fixture {
    invitation: PortableInvitation,
    request: PortableSpawnAdmissionRequest,
    manager: AdmissionManager,
    membership: BodyMembership,
}

fn fixture() -> Fixture {
    let body = Body::born(
        "source/documents".into(),
        "checked/documents".into(),
        1,
        SignId::from("sign/documents-born"),
    )
    .unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap(),
            [12; 32],
            NOW,
            EXPIRES,
        )
        .unwrap();
    let advertisement = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/document".into(),
        boot_id: "boot/document".into(),
        offer_generation: OfferGeneration(1),
        profile: "profile/document".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let signature = invitation.secret.sign(&invitation.signing_transcript(
        &advertisement.host_id,
        &advertisement.boot_id,
        advertisement.offer_generation,
    ));
    Fixture {
        request: PortableSpawnAdmissionRequest {
            schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: invitation.invitation_id.clone(),
            body_id: invitation.body_id.clone(),
            host_advertisement: advertisement,
            nonce: invitation.nonce,
            signature: signature.to_vec(),
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        },
        invitation: PortableInvitation {
            schema: INVITATION_SCHEMA.into(),
            claim: invitation.claim(),
            secret: invitation.secret.copy_for_target_provisioning(),
            rendezvous: None,
        },
        manager,
        membership: BodyMembership::new(body.body_id).unwrap(),
    }
}

fn routed(invitation: &mut PortableInvitation) {
    invitation.schema = ROUTED_INVITATION_SCHEMA.into();
    invitation.rendezvous = Some(SpawnRendezvousDescriptor {
        protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
        body_id: invitation.claim.body_id.as_str().into(),
        invitation_id: invitation.claim.invitation_id.as_str().into(),
        candidates: vec![RendezvousCandidate {
            candidate_id: "candidate/owner".into(),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: "wss://owner.example/body-admission".into(),
            authentication: RendezvousAuthentication {
                server_identity: "owner.example".into(),
                transport_binding_sha256: [13; 32],
            },
            expires_at_millis: EXPIRES,
            maximum_attempts: 1,
            attempt_timeout_millis: 1_000,
        }],
    });
}

fn signs() -> AdmissionSigns {
    AdmissionSigns {
        part_admitted: "sign/document-part".into(),
        host_attached: "sign/document-host".into(),
        candidate_admitted: "sign/document-candidate".into(),
    }
}

fn admitted(fixture: &mut Fixture) -> PortableAdmissionReceipt {
    let credential = fixture
        .manager
        .complete_spawn(
            &mut fixture.membership,
            &fixture.request.host_advertisement,
            &fixture.request.admission_proof().unwrap(),
            NOW + 1,
            signs(),
        )
        .unwrap();
    PortableAdmissionReceipt {
        schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential,
        host_advertisement: fixture.request.host_advertisement.clone(),
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
    }
}

fn exact_roundtrip<T: Serialize + DeserializeOwned>(document: &T, expected: Value) {
    let bytes = serde_json::to_vec(document).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), expected);
    let decoded: T = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(serde_json::to_vec(&decoded).unwrap(), bytes);
}

#[test]
fn existing_invitation_and_admission_json_shapes_roundtrip_exactly() {
    let mut f = fixture();
    exact_roundtrip(
        &f.invitation,
        json!({
            "schema": "conduit.body/spawn-invitation@1", "claim": f.invitation.claim,
            "secret": vec![11; 32],
        }),
    );
    routed(&mut f.invitation);
    exact_roundtrip(
        &f.invitation,
        json!({
            "schema": "conduit.body/spawn-invitation@2", "claim": f.invitation.claim,
            "secret": vec![11; 32], "rendezvous": f.invitation.rendezvous,
        }),
    );
    exact_roundtrip(
        &f.request,
        json!({
            "schema": "conduit.body/spawn-admission-request@1", "invitation_id": f.request.invitation_id,
            "body_id": f.request.body_id, "host_advertisement": f.request.host_advertisement,
            "nonce": vec![12; 32], "signature": f.request.signature,
            "membership_admitted": false, "plan_created": false, "play_created": false,
        }),
    );
    let receipt = admitted(&mut f);
    exact_roundtrip(
        &receipt,
        json!({
            "schema": "conduit.body/spawn-admission-receipt@1", "credential": receipt.credential,
            "host_advertisement": receipt.host_advertisement, "membership_admitted": true,
            "current_offers_available": false, "plan_created": false, "play_created": false,
        }),
    );
}

#[test]
fn legacy_routed_owner_frames_keep_the_existing_request_and_receipt_shapes() {
    assert_eq!(
        ROUTED_ADMISSION_REQUEST_SCHEMA,
        LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA
    );
    assert_eq!(
        ROUTED_ADMISSION_RESPONSE_SCHEMA,
        LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA
    );
    let mut f = fixture();
    let routed_request = RoutedAdmissionRequest {
        schema: LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: f.request.invitation_id.as_str().into(),
        request: f.request.clone(),
    };
    exact_roundtrip(
        &routed_request,
        json!({
            "schema": "conduit.body/routed-admission-request@1",
            "invitation_id": f.request.invitation_id,
            "request": f.request,
        }),
    );
    let receipt = admitted(&mut f);
    exact_roundtrip(
        &RoutedAdmissionResponse::Admitted {
            schema: LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA.into(),
            receipt: Box::new(receipt.clone()),
        },
        json!({
            "outcome": "admitted",
            "schema": "conduit.body/routed-admission-response@1",
            "receipt": receipt,
        }),
    );
    exact_roundtrip(
        &RoutedAdmissionResponse::Refused {
            schema: LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA.into(),
            code: "wrong-invitation".into(),
        },
        json!({
            "outcome": "refused",
            "schema": "conduit.body/routed-admission-response@1",
            "code": "wrong-invitation",
        }),
    );
    let mut unknown = serde_json::to_value(routed_request).unwrap();
    unknown["unknown"] = json!(true);
    assert!(serde_json::from_value::<RoutedAdmissionRequest>(unknown).is_err());
}

#[test]
fn existing_unknown_field_and_optional_route_policies_are_preserved() {
    let mut f = fixture();
    let mut invitation = serde_json::to_value(&f.invitation).unwrap();
    invitation["rendezvous"] = Value::Null;
    let decoded: PortableInvitation = serde_json::from_value(invitation.clone()).unwrap();
    assert!(decoded.rendezvous.is_none());
    invitation["unknown"] = json!(true);
    assert!(serde_json::from_value::<PortableInvitation>(invitation).is_err());
    let mut request = serde_json::to_value(&f.request).unwrap();
    request["unknown"] = json!(true);
    assert!(serde_json::from_value::<PortableSpawnAdmissionRequest>(request).is_err());
    let mut receipt = serde_json::to_value(admitted(&mut f)).unwrap();
    receipt["existing-extension"] = json!(true);
    let decoded: PortableAdmissionReceipt = serde_json::from_value(receipt).unwrap();
    decoded.validate_against(&f.request).unwrap();
}

#[test]
fn invitation_validation_keeps_route_identity_expiry_and_secret_distinct() {
    let mut invitation = fixture().invitation;
    invitation.validate(NOW).unwrap();
    routed(&mut invitation);
    invitation.validate(NOW).unwrap();
    for (change, expected) in [
        (0, Refusal::InvitationSchema),
        (1, Refusal::RouteSchema),
        (2, Refusal::RouteIdentity),
        (3, Refusal::RouteIdentity),
        (4, Refusal::Invitation(AdmissionRefusal::Expired)),
        (5, Refusal::Secret(AdmissionRefusal::WeakSecret)),
        (6, Refusal::Invitation(AdmissionRefusal::StaleNonce)),
    ] {
        let mut altered = invitation.clone();
        match change {
            0 => altered.schema = "conduit.body/spawn-invitation@99".into(),
            1 => altered.rendezvous = None,
            2 => altered.rendezvous.as_mut().unwrap().body_id = "body/other".into(),
            3 => altered.rendezvous.as_mut().unwrap().invitation_id = "invitation/other".into(),
            4 => altered.claim.expires_at_millis = NOW,
            5 => altered.secret = [0; 32],
            _ => altered.claim.nonce = [0; 32],
        }
        assert_eq!(altered.validate(NOW), Err(expected));
    }
    invitation.schema = INVITATION_SCHEMA.into();
    assert_eq!(invitation.validate(NOW), Err(Refusal::RouteSchema));
}

#[test]
fn request_conversion_preserves_exact_signature_and_cannot_assert_effects() {
    let request = fixture().request;
    let proof = request.admission_proof().unwrap();
    assert_eq!(proof.invitation_id, request.invitation_id);
    assert_eq!(proof.body_id, request.body_id);
    assert_eq!(proof.host_id, request.host_advertisement.host_id);
    assert_eq!(proof.boot_id, request.host_advertisement.boot_id);
    assert_eq!(proof.nonce, request.nonce);
    assert_eq!(proof.signature.as_slice(), request.signature);
    for change in 0..4 {
        let mut altered = request.clone();
        match change {
            0 => altered.schema = "conduit.body/spawn-admission-request@99".into(),
            1 => altered.membership_admitted = true,
            2 => altered.plan_created = true,
            _ => altered.play_created = true,
        }
        assert_eq!(
            altered.admission_proof().err(),
            Some(Refusal::RequestClaims)
        );
    }
    for length in [ADMISSION_SIGNATURE_BYTES - 1, ADMISSION_SIGNATURE_BYTES + 1] {
        let mut altered = request.clone();
        altered.signature.resize(length, 0);
        assert_eq!(
            altered.admission_proof().err(),
            Some(Refusal::SignatureBound)
        );
    }
}

#[test]
fn exact_receipt_requires_same_body_host_boot_offers_and_no_execution_claim() {
    let mut f = fixture();
    let receipt = admitted(&mut f);
    receipt.validate_against(&f.request).unwrap();
    for change in 0..8 {
        let mut altered = receipt.clone();
        match change {
            0 => altered.schema = "conduit.body/spawn-admission-receipt@99".into(),
            1 => altered.membership_admitted = false,
            2 => altered.plan_created = true,
            3 => altered.play_created = true,
            4 => altered.current_offers_available = true,
            5 => altered.credential.host_id = "host/other".into(),
            6 => altered.credential.boot_id = "boot/other".into(),
            _ => altered.host_advertisement.offer_generation = OfferGeneration(2),
        }
        assert_eq!(
            altered.validate_against(&f.request),
            Err(Refusal::ReceiptBasis)
        );
    }
    let mut altered_request = f.request.clone();
    altered_request.body_id = Body::born(
        "source/other".into(),
        "checked/other".into(),
        1,
        "sign/other".into(),
    )
    .unwrap()
    .body_id;
    assert_eq!(
        receipt.validate_against(&altered_request),
        Err(Refusal::ReceiptBasis)
    );
}

#[test]
fn documents_do_not_replace_signature_verification_or_consume_admission() {
    let mut f = fixture();
    f.invitation.validate(NOW).unwrap();
    f.request.validate().unwrap();
    let mut tampered = f.request.clone();
    tampered.signature[0] ^= 1;
    let proof = tampered.admission_proof().unwrap();
    let result = f.manager.complete_spawn(
        &mut f.membership,
        &tampered.host_advertisement,
        &proof,
        NOW + 1,
        signs(),
    );
    assert!(result.is_err());
    assert!(f.membership.parts.is_empty());
    let receipt = admitted(&mut f);
    receipt.validate_against(&f.request).unwrap();
    assert_eq!(f.membership.parts.len(), 1);
    assert_eq!(
        f.manager.complete_spawn(
            &mut f.membership,
            &f.request.host_advertisement,
            &f.request.admission_proof().unwrap(),
            NOW + 2,
            signs()
        ),
        Err(AdmissionRefusal::Replay)
    );
}

#[test]
fn version_two_route_reconstructs_the_complete_signed_request_and_legacy_refuses_mixed_profiles() {
    let mut f = fixture();
    let routed = RoutedAdmissionRequest {
        schema: COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: f.request.invitation_id.as_str().into(),
        request: f.request.clone(),
    };
    let value = serde_json::to_value(&routed).unwrap();
    assert_eq!(value["schema"], "conduit.body/routed-admission-request@2");
    assert_eq!(
        value["request"]["host_advertisement"]["encoding"],
        conduit_body::admission_advertisement_wire::ENCODING
    );
    let decoded: RoutedAdmissionRequest = serde_json::from_value(value.clone()).unwrap();
    assert!(decoded.supported_schema());
    assert_eq!(
        serde_json::to_value(&decoded.request).unwrap(),
        serde_json::to_value(&f.request).unwrap()
    );
    f.request = decoded.request;
    let _receipt = admitted(&mut f); // original signature completes actual invitation admission
    let mut mixed = value.clone();
    mixed["schema"] = json!(LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA);
    assert!(serde_json::from_value::<RoutedAdmissionRequest>(mixed).is_err());
    let mut mixed = value;
    mixed["request"]["host_advertisement"] =
        serde_json::to_value(f.request.host_advertisement).unwrap();
    assert!(serde_json::from_value::<RoutedAdmissionRequest>(mixed).is_err());
}

#[test]
fn routed_response_profile_preserves_exact_receipt_and_legacy_version_policy() {
    let mut f = fixture();
    let receipt = admitted(&mut f);
    let response = RoutedAdmissionResponse::Admitted {
        schema: COMPACT_ROUTED_ADMISSION_RESPONSE_SCHEMA.into(),
        receipt: Box::new(receipt.clone()),
    };
    let value = serde_json::to_value(response).unwrap();
    assert_eq!(
        value["receipt"]["host_advertisement"]["encoding"],
        conduit_body::admission_advertisement_wire::ENCODING
    );
    let decoded: RoutedAdmissionResponse = serde_json::from_value(value.clone()).unwrap();
    let RoutedAdmissionResponse::Admitted {
        schema,
        receipt: decoded,
    } = decoded
    else {
        panic!("admitted response")
    };
    assert!(conduit_body::supported_routed_admission_response_schema(
        &schema
    ));
    assert_eq!(
        serde_json::to_value(decoded).unwrap(),
        serde_json::to_value(&receipt).unwrap()
    );
    let mut mixed = value.clone();
    mixed["schema"] = json!(LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA);
    assert!(serde_json::from_value::<RoutedAdmissionResponse>(mixed).is_err());
    let mut mixed = value;
    mixed["receipt"]["host_advertisement"] =
        serde_json::to_value(receipt.host_advertisement).unwrap();
    assert!(serde_json::from_value::<RoutedAdmissionResponse>(mixed).is_err());
    for (request_schema, response_schema) in [
        (
            LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA,
            LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA,
        ),
        (
            COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA,
            COMPACT_ROUTED_ADMISSION_RESPONSE_SCHEMA,
        ),
    ] {
        let request = RoutedAdmissionRequest {
            schema: request_schema.into(),
            invitation_id: f.request.invitation_id.as_str().into(),
            request: f.request.clone(),
        };
        assert_eq!(request.response_schema(), response_schema);
        let refused = RoutedAdmissionResponse::Refused {
            schema: response_schema.into(),
            code: "no-admission".into(),
        };
        let value = serde_json::to_value(&refused).unwrap();
        let decoded: RoutedAdmissionResponse = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    }
}

#[path = "admission_documents/serialization_memory.rs"]
mod serialization_memory;
