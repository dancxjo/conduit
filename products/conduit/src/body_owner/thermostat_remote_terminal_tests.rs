//! Signed admission and real terminal/kernel execution on a running thermostat.
//! Directional TLS Lines here are contract fixtures, not live carrier evidence.
use super::*;
use conduit_body::{
    PortableSpawnAdmissionRequest, SpawnInvitationSecret, SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::*;
use conduit_face::OwnerFaceSnapshotRequest;
use conduit_std_host::hosted_terminal_mask_host::receive_terminal_frame_and_ack;
use conduit_std_host::terminal_face_mask::TerminalMaskExecution;
use std::os::unix::net::UnixStream;

#[test]
fn admitted_terminal_controls_the_same_running_thermostat_and_rejects_stale_show() {
    let (mut owner, root) = fixture(256);
    let mut peer = StdHost::new_with_config(StdHostConfig {
        host_id: "host/thermostat-terminal".into(),
        boot_id: "boot/thermostat-terminal".into(),
        offer_generation: OfferGeneration(1),
    });
    let (provider, mut foreground) = UnixStream::pair().unwrap();
    peer.attach_terminal_mask(provider).unwrap();
    let peer_offer = peer.advertisement().clone();
    let invitation = owner.issue_invitation(&root, 60, None).unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let receipt = owner
        .admit_invited(
            &root,
            PortableSpawnAdmissionRequest {
                schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
                invitation_id: invitation.claim.invitation_id.clone(),
                body_id: invitation.claim.body_id.clone(),
                host_advertisement: peer_offer.clone(),
                nonce: invitation.claim.nonce,
                signature: secret
                    .sign(&invitation.claim.signing_transcript(
                        &peer_offer.host_id,
                        &peer_offer.boot_id,
                        peer_offer.offer_generation,
                    ))
                    .to_vec(),
                membership_admitted: false,
                plan_created: false,
                play_created: false,
            },
            "host/thermostat-terminal",
        )
        .unwrap();
    let credential = &receipt.credential;
    let request = OwnerFaceSnapshotRequest {
        schema: conduit_face::OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: credential.credential_id.as_str().into(),
        body_id: credential.body_id.clone(),
        part_id: credential.part_id.clone(),
        host_id: credential.host_id.clone(),
        boot_id: credential.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    };
    let mut worker = owner.start_thermostat_run(&root, 30_000).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while owner.current_play_id().is_none() {
        assert!(Instant::now() < deadline);
        assert!(!worker.progress(&mut owner, &root).unwrap());
        thread::sleep(Duration::from_millis(5));
    }
    let play = owner.current_play_id().unwrap().clone();
    let body = owner.session.evidence().body_id.clone();
    let plan = owner.session.realization().unwrap().plan.plan_id.clone();
    let owner_offer = owner.host.advertisement().clone();
    let line = |name: &str, source: &HostAdvertisement, sink: &HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            &format!("line/thermostat-terminal-test/{name}"),
            &format!("binding/thermostat-terminal-test/{name}"),
            "conduit.base/authenticated-tls-websocket@1".into(),
            "base-instance/thermostat-terminal-test",
            source,
            sink,
            LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64 * 1024,
                maximum_buffered_bytes: 128 * 1024,
                maximum_frame_bytes: 64 * 1024,
            },
        );
        line.binding.credential =
            LinkCredentialReference::Opaque(credential.credential_id.as_str().into());
        line.binding.authority =
            LinkAuthorityReference::Grant("grant/thermostat-terminal-test".into());
        line.contract.scope = LineScope::RoutedNetwork;
        line.contract.security = LineSecurity::AuthenticatedEncrypted;
        line
    };
    let face_line = line("face", &owner_offer, &peer_offer);
    let show_line = line("show", &peer_offer, &owner_offer);
    let interaction_line = line("interaction", &peer_offer, &owner_offer);
    let expires = crate::durable_host::current_time_millis().unwrap() + 60_000;
    assert!(owner
        .seal_remote_terminal_mask_route(&receipt, &face_line, &show_line, &show_line, expires)
        .is_err());
    let mut forged = receipt.clone();
    forged.host_advertisement.profile = "fixture/changed-offer".into();
    assert!(owner
        .seal_remote_terminal_mask_route(
            &forged,
            &face_line,
            &show_line,
            &interaction_line,
            expires,
        )
        .is_err());
    let route = owner
        .seal_remote_terminal_mask_route(
            &receipt,
            &face_line,
            &show_line,
            &interaction_line,
            expires,
        )
        .unwrap();
    assert_eq!(owner.current_play_id(), Some(&play));
    let selected = owner
        .face_wardrobe
        .as_ref()
        .unwrap()
        .control()
        .selected
        .as_ref()
        .unwrap();
    assert_eq!(
        selected.route_id,
        format!("route/{}", route.route_plan_id.as_str())
    );
    let prior_control = owner.face_wardrobe.as_ref().unwrap().control().clone();
    assert!(owner
        .seal_remote_terminal_mask_route(&receipt, &face_line, &show_line, &show_line, expires,)
        .is_err());
    assert_eq!(
        owner.face_wardrobe.as_ref().unwrap().control(),
        &prior_control
    );
    let face = owner.face_snapshot(&request).unwrap();
    let retained = owner
        .pending_native_mask
        .as_ref()
        .unwrap()
        .current_witness(
            &owner.session,
            &face,
            crate::durable_host::current_time_millis().unwrap(),
        )
        .unwrap()
        .0;
    assert_eq!(retained, &route);
    let mut execution = peer.prepare_remote_terminal_mask_execution(&route).unwrap();
    let (release, wait) = std::sync::mpsc::sync_channel(1);
    let output = thread::spawn(move || {
        let mut bytes = Vec::new();
        let receipt = receive_terminal_frame_and_ack(&mut foreground, &mut bytes).unwrap();
        wait.recv().unwrap();
        (receipt, bytes)
    });
    let show = peer
        .represent_attached_terminal_face_with_execution(&face, &mut execution)
        .unwrap();
    assert!(owner.validate_native_mask_show(&request, &show).is_err());
    owner.acknowledge_native_mask_show(&request, &show).unwrap();
    owner.validate_native_mask_show(&request, &show).unwrap();
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == "thermostat.mode.heat")
        .unwrap();
    let interaction =
        FaceInteraction::new(&face, &show, &action.identity, &action.target, vec![], 1).unwrap();
    let mut oversized = interaction.clone();
    oversized
        .arguments
        .push(conduit_face::FaceInteractionArgument {
            name: "fixture/oversized".into(),
            value_kind: "Text".into(),
            value: vec![0; 64 * 1024],
        });
    assert!(owner
        .validate_native_mask_interaction(&request, &show, &oversized)
        .unwrap_err()
        .contains("ReturnExceedsLine"));
    let returned = execution.interact(interaction).unwrap();
    owner
        .validate_native_mask_interaction(&request, &show, &returned.interaction)
        .unwrap();
    worker
        .submit_interaction(&owner, &show, &returned.interaction)
        .unwrap();
    while worker.output_pending() {
        assert!(Instant::now() < deadline);
        assert!(!worker.progress(&mut owner, &root).unwrap());
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        owner.thermostat_live.as_ref().unwrap().state.mode,
        Mode::Heat
    );
    assert_eq!(owner.session.evidence().body_id, body);
    assert_eq!(owner.session.realization().unwrap().plan.plan_id, plan);
    assert_eq!(owner.current_play_id(), Some(&play));
    assert!(owner.host.is_playing());
    assert!(owner.validate_native_mask_show(&request, &show).is_err());
    assert!(worker
        .submit_interaction(&owner, &show, &returned.interaction)
        .is_err());
    release.send(()).unwrap();
    let (receipt, bytes) = output.join().unwrap();
    assert_eq!(receipt.bytes_written as usize, bytes.len());
    assert!(!bytes.is_empty());
    worker.request_lull().unwrap();
    while !worker.progress(&mut owner, &root).unwrap() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
}
