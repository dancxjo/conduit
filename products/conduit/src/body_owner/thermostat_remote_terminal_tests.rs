//! Signed admission and real terminal/kernel execution on a running thermostat.
//! Directional TLS Lines here are contract fixtures, not live carrier evidence.
use super::*;
use conduit_std_host::hosted_terminal_mask_host::receive_terminal_frame_and_ack;
use conduit_std_host::terminal_face_mask::TerminalMaskExecution;

#[test]
fn admitted_terminal_controls_the_same_running_thermostat_and_rejects_stale_show() {
    let (mut owner, root) = fixture(256);
    let (mut peer, mut foreground, receipt, request) =
        super::terminal_fixture::admitted_peer(&mut owner, &root);
    let peer_offer = peer.advertisement().clone();
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
    let [face_line, show_line, interaction_line] =
        super::terminal_fixture::terminal_lines(&owner_offer, &peer_offer, &receipt);
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
