//! Membership and Lines below are explicit contract fixtures. These tests run
//! the real terminal kernel and foreground provider; they are not live peer
//! admission, thermostat continuity, or transport acceptance evidence.
use super::*;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyLifecycleSession,
    BodyMembership, MembershipProofId, PartId,
};
use conduit_core::*;
use std::os::unix::net::UnixStream;

fn route(host: &StdHost, with_return: bool) -> (RemoteOwnerMaskRouteSeal, Face) {
    let planned = host.prepare_terminal_mask_execution().unwrap();
    let mask = &planned.planned_mask().mask;
    let body = Body::born(
        mask.plot_identity.source_document_id.clone(),
        mask.plot_identity.checked_plot_id.clone(),
        1,
        "sign/remote-terminal/born".into(),
    )
    .unwrap();
    let id = body.body_id.clone();
    let mut membership = BodyMembership::new(id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body, membership.clone(), "Remote terminal".into()).unwrap();
    let part = PartId::bind(&id, host.advertisement().host_id.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("remote-terminal/fixture").unwrap();
    let admitted = membership
        .admit(
            &id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "sign/remote-terminal/admitted".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.advertisement().host_id.clone(),
                boot_id: host.advertisement().boot_id.clone(),
                offer_generation: host.advertisement().offer_generation,
                proof_id: proof,
                sequence: 1,
            },
            "sign/remote-terminal/present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let session = BodyLifecycleSession::open(evidence).unwrap();
    let face = face(3)
        .with_basis(FaceBasis {
            body_id: Some(id),
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        })
        .unwrap();
    let owner = StdHost::new_with_config(crate::StdHostConfig {
        host_id: "host/remote-terminal-owner".into(),
        boot_id: "boot/remote-terminal-owner".into(),
        offer_generation: OfferGeneration(1),
    });
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 64 * 1024,
        maximum_buffered_bytes: 128 * 1024,
        maximum_frame_bytes: 64 * 1024,
    };
    let line = |label: &str, source: &HostAdvertisement, sink: &HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            &format!("line/remote-terminal/{label}"),
            &format!("binding/remote-terminal/{label}"),
            "conduit.base/websocket-rfc6455@1".into(),
            "base-instance/remote-terminal",
            source,
            sink,
            limits,
        );
        line.binding.credential =
            LinkCredentialReference::Opaque("credential/remote-terminal".into());
        line.binding.authority = LinkAuthorityReference::Grant("grant/remote-terminal".into());
        line.contract.scope = LineScope::RoutedNetwork;
        line.contract.security = LineSecurity::PlaintextNetwork;
        line
    };
    let input = line("face", owner.advertisement(), host.advertisement());
    let show = line("show", host.advertisement(), owner.advertisement());
    let interaction = line("interaction", host.advertisement(), owner.advertisement());
    let selected;
    let planned_mask = if with_return {
        let choices = [
            (PortDirection::Input, &mask.face_input.front_port_id, &input),
            (
                PortDirection::Output,
                &mask.show_output.front_port_id,
                &show,
            ),
            (
                PortDirection::Output,
                &mask.interaction_output.front_port_id,
                &interaction,
            ),
        ]
        .map(
            |(direction, port, line)| conduit_planner::ExternalForeLineChoice {
                host_id: host.advertisement().host_id.clone(),
                boot_id: host.advertisement().boot_id.clone(),
                direction,
                front_port_id: port.clone(),
                track: ConnectionTrack::Payload,
                peer_host_id: owner.advertisement().host_id.clone(),
                peer_boot_id: owner.advertisement().boot_id.clone(),
                line_id: line.line_id.clone(),
            },
        );
        let plan = conduit_planner::bind_external_fore_lines(
            &planned.planned_mask().plan,
            &choices,
            &[input.clone(), show.clone(), interaction.clone()],
            &["conduit.base/websocket-rfc6455@1".into()],
        )
        .unwrap();
        selected = conduit_face::PlannedMaskPlot::admit(mask, &plan).unwrap();
        &selected
    } else {
        planned.planned_mask()
    };
    let route = if with_return {
        RemoteOwnerMaskRouteSeal::seal_current_with_interaction(
            &session,
            &face,
            owner.advertisement(),
            host.advertisement(),
            planned_mask,
            &input,
            &show,
            &interaction,
        )
    } else {
        RemoteOwnerMaskRouteSeal::seal_current(
            &session,
            &face,
            owner.advertisement(),
            host.advertisement(),
            planned_mask,
            &input,
            &show,
        )
    }
    .unwrap();
    (route, face)
}

#[test]
fn owner_issued_remote_plan_executes_the_actual_terminal_show_and_return() {
    let mut host = StdHost::new();
    let (provider, mut foreground) = UnixStream::pair().unwrap();
    host.attach_terminal_mask(provider).unwrap();
    let (route, face) = route(&host, true);
    let mut execution = host.prepare_remote_terminal_mask_execution(&route).unwrap();
    assert_eq!(execution.planned_mask(), &route.planned_mask);
    let (release, wait) = std::sync::mpsc::sync_channel(1);
    let provider = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let receipt = receive_terminal_frame_and_ack(&mut foreground, &mut bytes).unwrap();
        wait.recv().unwrap();
        (receipt, bytes)
    });
    let show = host
        .represent_attached_terminal_face_with_execution(&face, &mut execution)
        .unwrap();
    assert_eq!(show.show.lifecycle, ManifestationLifecycle::Available);
    assert_eq!(
        show.planned_mask.plan.plan_id,
        route.planned_mask.plan.plan_id
    );
    assert_eq!(show.show.body_id, face.basis.body_id);
    let action = FaceInteraction::new(&face, &show, "start", "clock", vec![], 1).unwrap();
    let returned = execution.interact(action.clone()).unwrap();
    assert_eq!(returned.interaction, action);
    assert!(!execution.has_pending_play());
    release.send(()).unwrap();
    let (receipt, bytes) = provider.join().unwrap();
    assert_eq!(receipt.bytes_written as usize, bytes.len());
    assert!(String::from_utf8(bytes).unwrap().contains("A useful clock"));
}

#[test]
fn remote_terminal_refuses_missing_return_changed_face_and_retired_host() {
    let mut host = StdHost::new();
    let (provider, _foreground) = UnixStream::pair().unwrap();
    host.attach_terminal_mask(provider).unwrap();
    let (without_return, _) = route(&host, false);
    assert!(host
        .prepare_remote_terminal_mask_execution(&without_return)
        .is_err());
    let (route, face) = route(&host, true);
    let mut execution = host.prepare_remote_terminal_mask_execution(&route).unwrap();
    let mut changed = face.clone();
    changed.revision += 1;
    changed = changed.clone().with_basis(changed.basis.clone()).unwrap();
    assert!(execution.begin_render(&changed).is_err());
    assert!(!execution.has_pending_play());
    host.detach_terminal_mask().unwrap();
    assert!(execution
        .validate_current_host(host.advertisement())
        .is_err());
    assert!(host.prepare_remote_terminal_mask_execution(&route).is_err());
}
