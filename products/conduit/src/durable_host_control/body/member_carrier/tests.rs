use super::*;
use crate::durable_host_control::{handle, Request, Response, PROTOCOL};
use conduit_std_host::StdHost;
fn requests(token: u8) -> Vec<Request> {
    let ad = StdHost::new().advertisement().clone();
    let credential: MembershipCredential = serde_json::from_value(serde_json::json!({
        "credential_id":"fixture/credential", "body_id":"fixture/body", "part_id":"fixture/part",
        "host_id":ad.host_id, "boot_id":ad.boot_id, "issued_at_millis":1
    }))
    .unwrap();
    let peer = conduit_body::FederationPeer {
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        offer_generation: ad.offer_generation,
        verifying_key: [7; 32],
    };
    let challenge = FederationChallenge {
        session_id: "fixture/session".into(),
        line_id: "fixture/line".into(),
        line_security: LineSecurity::AuthenticatedEncrypted,
        initiator: peer.clone(),
        responder: peer,
        nonce: [9; 32],
        expires_at_millis: 100,
        maximum_frames: 2,
    };
    vec![
        Request::BodyMemberCarrierReceiver {
            protocol: PROTOCOL,
            token: vec![token; 32],
        },
        Request::BodyAttestAdmissionReceipt {
            protocol: PROTOCOL,
            token: vec![token; 32],
            receipt: Box::new(
                crate::durable_host::owner::MembershipReceiptDocument::Spawn(
                    conduit_body::PortableAdmissionReceipt {
                        schema: conduit_body::SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
                        credential: credential.clone(),
                        host_advertisement: ad.clone(),
                        membership_admitted: true,
                        current_offers_available: !ad.capabilities.is_empty(),
                        plan_created: false,
                        play_created: false,
                    },
                ),
            ),
            expected_owner: Box::new(challenge.responder.clone()),
        },
        Request::BodyAttestAdmissionReceipt {
            protocol: PROTOCOL,
            token: vec![token; 32],
            receipt: Box::new(
                crate::durable_host::owner::MembershipReceiptDocument::Return(
                    conduit_body::MemberReturnAcknowledgement::new(credential.clone(), ad.clone()),
                ),
            ),
            expected_owner: Box::new(challenge.responder.clone()),
        },
        Request::BodyMemberRecoverReturnBegin {
            protocol: PROTOCOL,
            token: vec![token; 32],
            credential: credential.clone(),
            advertisement: Box::new(ad.clone()),
            presentation: Box::new(conduit_body::MemberReturnImagePresentation {
                kind: "Advertise".into(),
                schema: conduit_body::MEMBER_RETURN_IMAGES_SCHEMA.into(),
                credential: credential.clone(),
                projection: conduit_body::HostAdvertisementImageProjection {
                    context: {
                        let mut context = ad.clone();
                        context.capabilities.clear();
                        context
                    },
                    capability_images: Vec::new(),
                    authority_grants: Vec::new(),
                },
                signature: vec![0; 64],
            }),
            images: Vec::new(),
        },
        Request::BodyMemberCarrierBegin {
            protocol: PROTOCOL,
            token: vec![token; 32],
            credential,
            advertisement: Box::new(ad),
            line: "fixture/line".into(),
            security: LineSecurity::AuthenticatedEncrypted,
            expires_at_millis: 100,
            maximum_frames: 2,
        },
        Request::BodyMemberCarrierComplete {
            protocol: PROTOCOL,
            token: vec![token; 32],
            challenge: Box::new(challenge),
            signature: vec![0; 64],
        },
        Request::BodyMemberPresenceRenew {
            protocol: PROTOCOL,
            token: vec![token; 32],
            session: "fixture/session".into(),
            sequence: 1,
        },
        Request::BodyMemberCarrierInspect {
            protocol: PROTOCOL,
            token: vec![token; 32],
            session: "fixture/session".into(),
        },
        Request::BodyMemberCarrierRetire {
            protocol: PROTOCOL,
            token: vec![token; 32],
            session: "fixture/session".into(),
        },
    ]
}
#[test]
fn previous_local_control_revision_refuses_before_selecting_a_carrier_owner() {
    let mut runtime = DurableHostRuntime::new(
        "fixture/target".into(),
        "fixture/image".into(),
        StdHost::new(),
    );
    let before = runtime.truth().advertisement;
    let request = Request::BodyMemberCarrierReceiver {
        protocol: PROTOCOL - 1,
        token: vec![9; 32],
    };
    assert!(
        matches!(handle(request,&[9;32],&mut runtime),Response::Refused{code,..} if code=="protocol")
    );
    assert_eq!(runtime.truth().advertisement, before);
}

#[test]
fn member_carrier_control_requires_authenticated_local_caller_and_real_body_owner() {
    let mut runtime = DurableHostRuntime::new(
        "fixture/target".into(),
        "fixture/image".into(),
        StdHost::new(),
    );
    let original = runtime.truth().advertisement;
    for token in [8, 9] {
        for request in requests(token) {
            // Actual typed wire round-trip; neither malformed credentials
            // nor a caller's transport description can create an owner.
            let encoded = serde_json::to_vec(&request).unwrap();
            let request = serde_json::from_slice(&encoded).unwrap();
            let recovery = matches!(&request, Request::BodyMemberRecoverReturnBegin { .. });
            let Response::Refused { code, .. } = handle(request, &[9; 32], &mut runtime) else {
                panic!("a control token cannot manufacture a live Body owner");
            };
            if token == 8 {
                assert_eq!(code, "unauthorized");
            } else if recovery {
                assert_eq!(code, "installed Host does not own a live Body session");
            } else {
                assert!(code.contains("live installed owner"), "{code}");
            }
        }
    }
    assert_eq!(runtime.truth().advertisement, original);
}
