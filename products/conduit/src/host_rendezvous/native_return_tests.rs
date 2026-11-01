use super::*;
use conduit_body::*;

fn fixture() -> (NativeReturn, AdmissionManager, BodyMembership) {
    let body = Body::born(
        "source/native-return".into(),
        "checked/native-return".into(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([5; 32]).unwrap(),
            [6; 32],
            100,
            1000,
        )
        .unwrap();
    let mut ad = conduit_std_host::StdHost::new().advertisement().clone();
    let claim = SpawnInvitationClaim {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        nonce: invitation.nonce,
        expires_at_millis: invitation.expires_at_millis,
    };
    let signature = invitation.secret.sign(&claim.signing_transcript(
        &ad.host_id,
        &ad.boot_id,
        ad.offer_generation,
    ));
    let credential = manager
        .complete_spawn(
            &mut membership,
            &ad,
            &SpawnAdmissionProof {
                invitation_id: claim.invitation_id.clone(),
                body_id: claim.body_id.clone(),
                host_id: ad.host_id.clone(),
                boot_id: ad.boot_id.clone(),
                nonce: claim.nonce,
                signature,
            },
            101,
            AdmissionSigns {
                part_admitted: "sign/part".into(),
                host_attached: "sign/attach".into(),
                candidate_admitted: "sign/candidate".into(),
            },
        )
        .unwrap();
    let document = serde_json::json!({
        "schema":"conduit.native/part-custody@1",
        "pending": {"schema":"conduit.body/pending-join@1",
            "invitation":{"schema":INVITATION_SCHEMA,"claim":claim,"secret":vec![5;32],
                "rendezvous":{"protocol":1,"body_id":body.body_id,"invitation_id":invitation.invitation_id,"candidates":[{"candidate_id":"route/native-return","line_family":"authenticated-tls-stream","reachability":"wss://localhost:8448/return","authentication":{"server_identity":"localhost","transport_binding_sha256":vec![8;32]},"expires_at_millis":1000,"maximum_attempts":1,"attempt_timeout_millis":1000}]}},
            "request":{"schema":SPAWN_ADMISSION_REQUEST_SCHEMA,"invitation_id":claim.invitation_id,"body_id":claim.body_id,
                "host_advertisement":ad,"nonce":claim.nonce,"signature":signature.to_vec(),"membership_admitted":false,"plan_created":false,"play_created":false}},
        "receipt":{"schema":SPAWN_ADMISSION_RECEIPT_SCHEMA,"credential":credential,"host_advertisement":ad,
            "current_offers_available":!ad.capabilities.is_empty(),"membership_admitted":true,"plan_created":false,"play_created":false}
    });
    membership
        .observe_offline(
            &body.body_id,
            membership.revision,
            &credential.part_id,
            &ad.boot_id,
            "sign/offline".into(),
        )
        .unwrap();
    ad.boot_id = "boot/native-return-fresh".into();
    ad.offer_generation = conduit_core::OfferGeneration(27);
    let retained = RetainedPart::from_test_document(&serde_json::to_vec(&document).unwrap(), ad);
    (NativeReturn::new(retained).unwrap(), manager, membership)
}
fn encoded(out: Out) -> Vec<u8> {
    serde_json::to_vec(&out).unwrap()
}

#[test]
fn canonical_owner_verifies_retained_signer_for_exact_fresh_boot() {
    let (mut client, mut manager, mut membership) = fixture();
    let request: In = serde_json::from_slice(&client.advertise().unwrap()).unwrap();
    let In::ReturnAdvertise {
        protocol: PROTOCOL,
        credential,
        advertisement,
    } = request
    else {
        panic!("wrong canonical request")
    };
    assert_eq!(client.rendezvous().body_id, membership.body_id.as_str());
    let challenge = manager
        .begin_return(
            &membership,
            &credential.part_id,
            &advertisement,
            [9; 32],
            200,
            1000,
        )
        .unwrap();
    let reply = client
        .challenge(&encoded(Out::ReturnChallenge {
            protocol: PROTOCOL,
            challenge: challenge.clone(),
        }))
        .unwrap();
    let In::ReturnProof {
        protocol: PROTOCOL,
        admission_id,
        body_id,
        part_id,
        host_id,
        boot_id,
        nonce,
        signature,
    } = serde_json::from_slice(&reply).unwrap()
    else {
        panic!("wrong canonical proof")
    };
    let proof = PartReturnProof {
        admission_id,
        body_id,
        part_id,
        host_id,
        boot_id,
        nonce: nonce.try_into().unwrap(),
        signature: signature.try_into().unwrap(),
    };
    let returned = manager
        .complete_return(
            &mut membership,
            &advertisement,
            &proof,
            201,
            "sign/returned".into(),
        )
        .unwrap();
    assert_eq!(returned.part_id, credential.part_id);
    assert_eq!(returned.boot_id, advertisement.boot_id);
    let accepted = client
        .admitted(&encoded(Out::Admitted {
            protocol: PROTOCOL,
            credential: returned.clone(),
            signed_membership_receipt: None,
        }))
        .unwrap();
    assert_eq!(accepted, returned);
    assert!(client.admitted(b"{}").is_err());
    assert!(manager
        .complete_return(
            &mut membership,
            &advertisement,
            &proof,
            202,
            "sign/replay".into()
        )
        .is_err());
}
#[test]
fn wrong_boot_generation_and_oversize_are_terminal_before_signing() {
    for wrong_generation in [false, true] {
        let (mut client, mut manager, membership) = fixture();
        client.advertise().unwrap();
        let mut c = manager
            .begin_return(
                &membership,
                &client.part.credential().part_id,
                client.part.advertisement(),
                [9; 32],
                200,
                1000,
            )
            .unwrap();
        if wrong_generation {
            c.offer_generation = conduit_core::OfferGeneration(28);
        } else {
            c.boot_id = "boot/foreign".into();
        }
        assert!(client
            .challenge(&encoded(Out::ReturnChallenge {
                protocol: PROTOCOL,
                challenge: c
            }))
            .is_err());
        assert!(client.challenge(b"{}").is_err());
    }
    let (mut client, _, _) = fixture();
    client.advertise().unwrap();
    assert!(client.challenge(&vec![b' '; LIMIT + 1]).is_err());
    assert!(client.challenge(b"{}").is_err());
}

#[test]
fn wrong_scope_and_wrong_protocol_never_produce_proof() {
    for field in ["body_id", "part_id", "host_id"] {
        let (mut client, mut manager, membership) = fixture();
        let advertisement = client.advertise().unwrap();
        let public: serde_json::Value = serde_json::from_slice(&advertisement).unwrap();
        assert!(public.get("secret").is_none());
        assert!(public.get("invitation").is_none());
        let challenge = manager
            .begin_return(
                &membership,
                &client.part.credential().part_id,
                client.part.advertisement(),
                [9; 32],
                200,
                1000,
            )
            .unwrap();
        let mut wire = serde_json::to_value(Out::ReturnChallenge {
            protocol: PROTOCOL,
            challenge,
        })
        .unwrap();
        wire["challenge"][field] = serde_json::json!("foreign/scope");
        assert!(client
            .challenge(&serde_json::to_vec(&wire).unwrap())
            .is_err());
        assert!(client.advertise().is_err());
    }
    let (mut client, _, _) = fixture();
    client.advertise().unwrap();
    assert!(client
        .challenge(br#"{"kind":"refused","protocol":2,"code":"no"}"#)
        .is_err());
}
