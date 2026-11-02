//! Real local pinned TLS plus Core owner admission and the retained private signer.
//! The first completion commits but its response is dropped; no Admitted frame
//! is fabricated, and no remote network/physical display claim is made.
use super::*;
use crate::durable_host as installed;
use crate::durable_host::routed_join::{MemberReturnFrame, MEMBER_RETURN_SCHEMA};
use conduit_body::{PortableSpawnAdmissionRequest, RendezvousAuthentication, RendezvousLineFamily};
use conduit_std_host::secure_websocket::{SecureWebSocketLine, SecureWebSocketListener};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use std::fs;

fn server_receive(line: &mut SecureWebSocketLine) -> MemberReturnFrame {
    let mut bytes = vec![0; ADMISSION];
    let length = line.receive_binary(&mut bytes).unwrap();
    serde_json::from_slice(&bytes[..length]).unwrap()
}
fn server_send(line: &mut SecureWebSocketLine, frame: &MemberReturnFrame) {
    line.send_binary(&serde_json::to_vec(frame).unwrap())
        .unwrap();
}

#[test]
fn signed_return_recovers_lost_committed_reply_over_real_pinned_tls_once() {
    let (mut owner, owner_state) = installed::owner::member_return_owner_fixture();
    let (manifest, state) = installed::tests::fixture();
    let installation = installed::install_without_start(&manifest, &state).unwrap();
    // A finite identity-only test Host; no peripheral or execution offers.
    let old = conduit_core::HostAdvertisement {
        declared_name: None,
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: installation.host_id.as_str().into(),
        boot_id: "boot/member-tls/old".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "test/member-return".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let invitation = owner.issue_invitation(&owner_state, 60, None).unwrap();
    let key = conduit_body::SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: old.clone(),
        nonce: invitation.claim.nonce,
        signature: key
            .sign(&invitation.claim.signing_transcript(
                &old.host_id,
                &old.boot_id,
                old.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let pending = installed::invitation::PendingBodyJoin {
        schema: "conduit.body/pending-join@1".into(),
        invitation,
        request: request.clone(),
    };
    fs::create_dir_all(state.join("body")).unwrap();
    installed::restrict_directory(&state.join("body")).unwrap();
    installed::write_json_atomic(&state.join("body/pending-join.json"), &pending).unwrap();
    let receipt = owner
        .admit_invited(&owner_state, request, old.host_id.as_str())
        .unwrap();
    installed::membership::complete_body_join_document(receipt.clone(), &state, true).unwrap();
    let detached = owner
        .member_leave(&owner_state, &receipt.credential)
        .unwrap();
    let mut actual = old;
    actual.boot_id = "boot/member-tls/returned".into();
    actual.offer_generation = conduit_core::OfferGeneration(2);

    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let certificate = state.join("tls-cert.pem");
    let private_key = state.join("tls-key.pem");
    fs::write(&certificate, cert.pem()).unwrap();
    fs::write(&private_key, signing_key.serialize_pem()).unwrap();
    // The network listener deliberately refuses loopback classification and
    // port zero. Reserve a free port, then bind its actual authorized wildcard.
    let reserved = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let bind = reserved.local_addr().unwrap();
    drop(reserved);
    let listener =
        SecureWebSocketListener::bind(bind, &certificate, &private_key, ADMISSION as u32, true)
            .unwrap();
    let address = listener.local_addr().unwrap();
    let candidate = RendezvousCandidate {
        candidate_id: "candidate/member-return-tls-test".into(),
        line_family: RendezvousLineFamily::AuthenticatedTlsStream,
        reachability: format!("wss://127.0.0.1:{}/member", address.port()),
        authentication: RendezvousAuthentication {
            server_identity: "127.0.0.1".into(),
            transport_binding_sha256: listener.certificate_binding_sha256(),
        },
        expires_at_millis: installed::current_time_millis().unwrap() + 10_000,
        maximum_attempts: 2,
        attempt_timeout_millis: 3_000,
    };
    let prior = receipt.credential;
    let before = detached.membership.revision;
    let mut accepted = None;
    std::thread::scope(|scope| {
        let client = scope.spawn(|| {
            let mut wrong_pin = candidate.clone();
            wrong_pin.authentication.transport_binding_sha256[0] ^= 1;
            assert!(connect(&wrong_pin, Duration::from_secs(3)).is_err());
            let (_line, returned) =
                bounded_signed_return(&state, &prior, &actual, &candidate).unwrap();
            returned
        });
        // TLS pin refusal occurs before any owner challenge or completion.
        assert!(listener
            .accept_with_timeout(Duration::from_secs(3))
            .is_err());
        for attempt in 0..2 {
            let mut line = listener
                .accept_with_timeout(Duration::from_secs(3))
                .unwrap();
            line.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let MemberReturnFrame::Advertise {
                schema,
                credential,
                advertisement,
            } = server_receive(&mut line)
            else {
                panic!("expected actual advertisement")
            };
            assert_eq!(schema, MEMBER_RETURN_SCHEMA);
            assert!(credential == prior && *advertisement == actual);
            let challenge = owner
                .begin_member_return(&credential, &advertisement)
                .unwrap();
            server_send(
                &mut line,
                &MemberReturnFrame::Challenge {
                    schema: MEMBER_RETURN_SCHEMA.into(),
                    challenge: Box::new(challenge),
                },
            );
            let MemberReturnFrame::Proof { schema, proof } = server_receive(&mut line) else {
                panic!("expected real signed proof")
            };
            assert_eq!(schema, MEMBER_RETURN_SCHEMA);
            let returned = owner
                .complete_member_return(&owner_state, &proof.decode().unwrap())
                .unwrap();
            if attempt == 0 {
                accepted = Some(returned);
                drop(line); // Deliberate lost response after durable commit.
            } else {
                assert!(accepted.as_ref() == Some(&returned));
                server_send(
                    &mut line,
                    &MemberReturnFrame::Admitted {
                        schema: MEMBER_RETURN_SCHEMA.into(),
                        credential: returned,
                        advertisement,
                    },
                );
            }
        }
        assert!(Some(client.join().unwrap()) == accepted);
    });
    let returned = accepted.unwrap();
    installed::membership::retain_returned_membership(&state, &prior, &returned, &actual).unwrap();
    let mut current_candidate = candidate.clone();
    current_candidate.expires_at_millis = installed::current_time_millis().unwrap() + 60_000;
    let current_encounter = installed::routed_join::MemberEncounter {
        schema: installed::routed_join::MEMBER_ROUTE_SCHEMA.into(),
        body_id: returned.body_id.clone(),
        part_id: returned.part_id.clone(),
        host_id: returned.host_id.clone(),
        candidate: current_candidate.clone(),
        owner: owner.prepare_current_member_carrier_receiver().unwrap(),
    };
    let biography_before = owner.truth()["biography"].clone();
    let admission_path = owner_state.join("body/admission.json");
    let admission_before = fs::read(&admission_path).unwrap();
    let mut previous_session = None;
    for _ in 0..2 {
        let controller = std::cell::RefCell::new(&mut owner);
        std::thread::scope(|scope| {
            let client = scope.spawn(|| {
                let mut line = connect(&current_candidate, Duration::from_secs(3)).unwrap();
                installed::routed_join::current_member::resume(
                    &state,
                    &returned,
                    &actual,
                    &current_encounter,
                    &mut line,
                )
                .unwrap()
            });
            let mut line = listener
                .accept_with_timeout(Duration::from_secs(3))
                .unwrap();
            line.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let frame = server_receive(&mut line);
            let (_, session) = installed::routed_join::current_member::serve_with(
                &current_encounter,
                &mut line,
                frame,
                |credential, ad, label, millis| {
                    controller.borrow_mut().begin_current_member_carrier(
                        &credential,
                        &ad,
                        label,
                        conduit_core::LineSecurity::AuthenticatedEncrypted,
                        millis,
                        128,
                    )
                },
                |challenge, signature| {
                    controller
                        .borrow_mut()
                        .complete_current_member_carrier(&challenge, signature)
                },
                |session| {
                    controller
                        .borrow_mut()
                        .retire_current_member_carrier(&session)
                },
            )
            .unwrap();
            assert_eq!(client.join().unwrap().session_id, session);
            assert!(previous_session.as_ref().is_none_or(|old| old != &session));
            controller
                .borrow_mut()
                .retire_current_member_carrier(&session)
                .unwrap();
            previous_session = Some(session);
        });
    }
    assert_eq!(owner.truth()["biography"], biography_before);
    assert_eq!(fs::read(&admission_path).unwrap(), admission_before);
    let withdrawn = owner.member_leave(&owner_state, &returned).unwrap();
    // Exactly one attachment followed by this explicit test withdrawal.
    assert_eq!(withdrawn.membership.revision.0, before.0 + 2);
    let reopened = installed::read_installation(&state.join("installation.json")).unwrap();
    assert!(installed::membership::load_joined_credential(&state, &reopened).unwrap() == returned);
    // The finite route is installed through its checked operation, not by
    // editing retained state. Selection itself creates no membership or Show.
    let descriptor = state.join("selected-member-encounter.json");
    let encounter = installed::routed_join::MemberEncounter {
        schema: installed::routed_join::MEMBER_ROUTE_SCHEMA.into(),
        body_id: returned.body_id.clone(),
        part_id: returned.part_id.clone(),
        host_id: returned.host_id.clone(),
        candidate: candidate.clone(),
        owner: owner.prepare_current_member_carrier_receiver().unwrap(),
    };
    installed::write_json_atomic(&descriptor, &encounter).unwrap();
    assert!(installed::routed_join::install_member_encounter(&state, &descriptor, false).is_err());
    installed::routed_join::install_member_encounter(&state, &descriptor, true).unwrap();
    let selected = fs::read(state.join("body/member-encounter.json")).unwrap();
    let held = installed::owner_lock::acquire(&state).unwrap();
    assert!(installed::routed_join::install_member_encounter(&state, &descriptor, true).is_err());
    drop(held);
    let mut forged = encounter;
    forged.host_id = "host/another-member".into();
    installed::write_json_atomic(&descriptor, &forged).unwrap();
    assert!(installed::routed_join::install_member_encounter(&state, &descriptor, true).is_err());
    assert_eq!(
        fs::read(state.join("body/member-encounter.json")).unwrap(),
        selected
    );
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
    fs::remove_dir_all(owner_state).unwrap();
}
