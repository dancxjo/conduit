use super::*;
use alloc::boxed::Box;
use conduit_body::{Body, MembershipCredential, SPAWN_ADMISSION_RECEIPT_SCHEMA};
use conduit_core::{OfferGeneration, PROTOCOL_VERSION, SignId};
use conduit_face::{BodyFace, FaceContext, FaceFocus, OWNER_FACE_RESPONSE_SCHEMA};

#[test]
fn refresh_requires_exact_credential_and_admitted_current_face() {
    let body = Body::born(
        "source/native-refresh".into(),
        "checked/native-refresh".into(),
        1,
        SignId::from("sign/native-refresh-born"),
    )
    .unwrap();
    let advertisement = conduit_core::HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/native-refresh".into(),
        boot_id: "boot/native-refresh".into(),
        offer_generation: OfferGeneration(1),
        profile: "profile/native-refresh".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let credential = MembershipCredential {
        credential_id: serde_json::from_str("\"credential/native-refresh\"").unwrap(),
        body_id: body.body_id.clone(),
        part_id: serde_json::from_str("\"part/native-refresh\"").unwrap(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        issued_at_millis: 1,
    };
    let receipt = PortableAdmissionReceipt {
        schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential: credential.clone(),
        host_advertisement: advertisement,
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
    };
    let face = BodyFace::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .face;
    let snapshot = || OwnerFaceSnapshotResponse::Snapshot {
        schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
        face: Box::new(face.clone()),
        interactions_admitted: false,
        route: None,
    };
    let prior = NativeGuestFace::from_owner_response(&receipt, snapshot()).unwrap();
    let grant = NativeOwnerReturnGrant {
        schema: "conduit.body/native-owner-return-grant@1".into(),
        token: [7; 32],
        credential_id: credential.credential_id.as_str().into(),
        body_id: credential.body_id.as_str().into(),
        part_id: credential.part_id.as_str().into(),
        host_id: credential.host_id.as_str().into(),
        boot_id: credential.boot_id.as_str().into(),
        remaining_millis: 60_000,
        maximum_actions: 4,
    };
    assert!(matches!(
        validated_refresh(
            &receipt,
            &prior,
            RefreshResponse {
                schema: REFRESH_RESPONSE_SCHEMA.into(),
                face: OwnerFaceSnapshotResponse::Refused {
                    schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                    code: "selected-native-route-unavailable".into(),
                },
                grant: None,
            },
        ),
        Err("native-owner-refresh-owner-refused")
    ));
    assert!(matches!(
        validated_refresh(
            &receipt,
            &prior,
            RefreshResponse {
                schema: REFRESH_RESPONSE_SCHEMA.into(),
                face: snapshot(),
                grant: None,
            },
        ),
        Err("native-owner-refresh-grant-absent")
    ));
    let mut foreign = grant.clone();
    foreign.boot_id = "boot/foreign".into();
    assert!(matches!(
        validated_refresh(
            &receipt,
            &prior,
            RefreshResponse {
                schema: REFRESH_RESPONSE_SCHEMA.into(),
                face: snapshot(),
                grant: Some(foreign),
            },
        ),
        Err("native-owner-refresh-grant-invalid")
    ));
    assert!(matches!(
        validated_refresh(
            &receipt,
            &prior,
            RefreshResponse {
                schema: REFRESH_RESPONSE_SCHEMA.into(),
                face: snapshot(),
                grant: Some(grant),
            },
        ),
        Err("native-owner-refresh-face-stale-or-route-changed")
    ));
}
