use super::*;
use crate::durable_host::owner::Owner;
use crate::durable_host_control::{Request, Response};
use conduit_body::ResidentPlot;
use conduit_std_host::{hosted_terminal_mask_host::receive_terminal_frame_and_ack, StdHost};
use std::{
    fs,
    io::Read,
    os::unix::net::UnixListener,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
};

static NEXT_STATE: AtomicU64 = AtomicU64::new(0);

fn runtime() -> DurableHostRuntime {
    let checked =
        crate::plot_source::parse(include_str!("../../../../../plots/clock/main.conduit"))
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap();
    let resident = ResidentPlot::new(
        checked.expanded.source_document_id.clone(),
        checked.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(StdHost::new(), resident, None, "Test clock").unwrap();
    owner.set_resident_plot_name(&checked).unwrap();
    let mut runtime = DurableHostRuntime::new("test".into(), "test".into(), StdHost::new());
    runtime.host = HostSource::Body {
        owner: Box::new(owner),
        root: PathBuf::new(),
        running: None,
    };
    runtime
}

fn request(runtime: &DurableHostRuntime, token: &[u8; 32]) -> AttachRequest {
    let HostSource::Body { owner, .. } = &runtime.host else {
        unreachable!()
    };
    let face = owner.local_face_snapshot().unwrap();
    let advertised = owner.host.advertisement();
    AttachRequest {
        protocol: PROTOCOL,
        token: token.to_vec(),
        body_id: face.basis.body_id.unwrap(),
        host_id: advertised.host_id.clone(),
        boot_id: advertised.boot_id.clone(),
        offer_generation: advertised.offer_generation,
        face_id: face.identity,
        face_revision: face.revision,
    }
}

fn state(runtime: &DurableHostRuntime) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "conduit-terminal-attachment-{}-{}",
        std::process::id(),
        NEXT_STATE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    let advertised = runtime.host.advertisement();
    fs::write(
        path.join("runtime.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema":"conduit.install/durable-host-runtime@1",
            "host_id":advertised.host_id.as_str(),
            "boot_id":advertised.boot_id.as_str(),
            "offer_generation":advertised.offer_generation.0,
            "process_id":std::process::id(),
            "release_bundle_sha256":"test",
            "body_id":null
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

#[test]
fn current_lulled_owner_attaches_actual_host_and_retains_interactive_show() {
    let mut runtime = runtime();
    let state = state(&runtime);
    let token = [7; 32];
    let before = runtime.host.advertisement().clone();
    let expected = before.clone();
    let request = request(&runtime, &token);
    let (mut client, mut server) = UnixStream::pair().unwrap();
    let worker = thread::spawn(move || {
        wire::write_request(&mut client, &request).unwrap();
        let reply = wire::read_reply(&mut client).unwrap();
        assert!(
            matches!(reply, AttachReply::Attached { protocol: PROTOCOL }),
            "{reply:?}"
        );
        let mut output = Vec::new();
        let receipt = receive_terminal_frame_and_ack(&mut client, &mut output).unwrap();
        let AttachReply::Show {
            protocol: PROTOCOL,
            route_plan_id,
            show,
            advertisement,
        } = wire::read_reply(&mut client).unwrap()
        else {
            panic!("expected acknowledged owner Show");
        };
        assert!(!output.is_empty());
        assert_eq!(receipt.bytes_written as usize, output.len());
        assert_eq!(advertisement.host_id, expected.host_id);
        assert_eq!(advertisement.boot_id, expected.boot_id);
        assert!(advertisement.offer_generation > expected.offer_generation);
        assert_eq!(
            show.planned_mask.plan.fragments[0].host_id,
            expected.host_id
        );
        assert_eq!(
            show.planned_mask.plan.fragments[0].boot_id,
            expected.boot_id
        );
        (client, route_plan_id, *show)
    });
    let mut first = [0];
    server.read_exact(&mut first).unwrap();
    serve(&state, &mut server, &mut runtime, &token, first[0]).unwrap();
    let (client, route_plan_id, show) = worker.join().unwrap();
    assert!(is_attached(&mut runtime));
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(state.join("runtime.json")).unwrap()).unwrap();
    assert_eq!(
        marker["offer_generation"],
        runtime.host.advertisement().offer_generation.0
    );
    let route = runtime.terminal_route.as_ref().unwrap();
    assert_eq!(route.seal.route_plan_id, route_plan_id);
    assert_eq!(route.show, show);
    assert_eq!(route.wardrobe.active_plan_id, route_plan_id);
    assert_eq!(
        route.wardrobe.selected.as_ref().unwrap().mask_plot,
        route.seal.planned_mask.mask.plot_identity
    );
    assert!(route.wardrobe.scoped_wardrobe.wake_id.is_none());
    assert!(route.execution.has_pending_play());
    assert!(matches!(
        route.show.show.lifecycle,
        conduit_presentation::ManifestationLifecycle::Available
    ));
    let HostSource::Body { owner, .. } = &runtime.host else {
        unreachable!()
    };
    let current_face = owner.local_face_snapshot().unwrap();
    assert!(show.validate(&current_face).is_ok());
    assert!(current_face.basis.wake_id.is_none());
    let admitted = owner
        .admit_attached_terminal_show(&route.seal, &route.show)
        .unwrap();
    assert_eq!(admitted.body_id(), &route.seal.body_id);
    assert_eq!(admitted.plan_id(), &route_plan_id);
    let wrong_body = conduit_presentation::BodyMaskWardrobe::new(
        serde_json::from_str("\"another-body\"").unwrap(),
        None,
        conduit_presentation::MaskWardrobe::new(
            conduit_presentation::MaskWardrobeLifetime::Body,
            vec![route.seal.planned_mask.mask.plot_identity.clone()],
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        conduit_presentation::MaskWardrobeControl::new_from_admitted_routes(
            wrong_body, &admitted, None,
        ),
        Err(conduit_presentation::MaskWardrobeControlError::WrongBody)
    );
    let mut forged_show = route.show.clone();
    forged_show.show.offer_generation.0 += 1;
    assert!(owner
        .admit_attached_terminal_show(&route.seal, &forged_show)
        .unwrap_err()
        .contains("OwnerRoute(InvalidShow)"));
    assert!(current_face.actions.iter().any(|action| {
        action.intent == crate::durable_host::owner::clock_interval_action()
            && action.availability.is_available()
    }));
    assert_eq!(
        runtime.start_owned_body(1_000).unwrap_err(),
        "terminal-attachment-must-detach-before-start"
    );
    drop(client);
    retire_closed_attachment(&state, &mut runtime).unwrap();
    assert!(!is_attached(&mut runtime));
    assert!(runtime.terminal_route.is_none());
    assert!(runtime.host.advertisement().offer_generation > before.offer_generation);
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(state.join("runtime.json")).unwrap()).unwrap();
    assert_eq!(
        marker["offer_generation"],
        runtime.host.advertisement().offer_generation.0
    );
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn owner_wardrobe_doff_and_rewear_do_not_reuse_the_old_show() {
    let mut runtime = runtime();
    let state = state(&runtime);
    let token = [7; 32];
    let request = request(&runtime, &token);
    let (mut client, mut server) = UnixStream::pair().unwrap();
    let worker = thread::spawn(move || {
        wire::write_request(&mut client, &request).unwrap();
        assert!(matches!(
            wire::read_reply(&mut client).unwrap(),
            AttachReply::Attached { .. }
        ));
        receive_terminal_frame_and_ack(&mut client, &mut Vec::new()).unwrap();
        let AttachReply::Show {
            route_plan_id,
            show,
            ..
        } = wire::read_reply(&mut client).unwrap()
        else {
            panic!("expected attached terminal Show");
        };
        (client, route_plan_id, *show)
    });
    let mut first = [0];
    server.read_exact(&mut first).unwrap();
    serve(&state, &mut server, &mut runtime, &token, first[0]).unwrap();
    let (client, plan, show) = worker.join().unwrap();
    fs::write(state.join("control.token"), token).unwrap();
    let listener = UnixListener::bind(state.join("control.sock")).unwrap();
    let server = thread::spawn(move || {
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().unwrap();
            let request: Request = crate::durable_host_control::read_frame(&mut stream).unwrap();
            let response = crate::durable_host_control::handle(request, &token, &mut runtime);
            crate::durable_host_control::write_frame(&mut stream, &response).unwrap();
        }
        runtime
    });
    let wardrobe_call = |plan: &PlanId, revision, command| {
        crate::durable_host_control::body::call(
            &state,
            Request::BodyAttachedTerminalWardrobe {
                protocol: PROTOCOL,
                token: token.to_vec(),
                route_plan_id: plan.clone(),
                show: Box::new(show.clone()),
                basis_revision: revision,
                command,
            },
        )
        .unwrap()
    };
    let Response::BodyAttachedTerminalWardrobe {
        report: initial, ..
    } = wardrobe_call(&plan, 0, TerminalWardrobeCommand::Inspect)
    else {
        panic!("owner must return a wardrobe report");
    };
    assert_eq!(initial["wardrobe"]["revision"], 0);
    assert_eq!(initial["admitted_routes"].as_array().unwrap().len(), 1);
    assert_eq!(initial["show_id"], show.show_id.as_str());
    assert!(matches!(
        wardrobe_call(
            &PlanId::from("wrong-plan"),
            0,
            TerminalWardrobeCommand::Doff
        ),
        Response::Refused { .. }
    ));
    let Response::BodyAttachedTerminalWardrobe { report: doffed, .. } =
        wardrobe_call(&plan, 0, TerminalWardrobeCommand::Doff)
    else {
        panic!("owner must return a doff report");
    };
    assert_eq!(doffed["wardrobe"]["revision"], 1);
    assert!(doffed["wardrobe"]["worn"].as_array().unwrap().is_empty());
    assert!(doffed["selected"].is_null());
    assert!(doffed["show_id"].is_null());
    assert_eq!(doffed["reconciliation"]["planning"], "NotRequired");
    assert!(matches!(
        wardrobe_call(&plan, 0, TerminalWardrobeCommand::Wear),
        Response::Refused { .. }
    ));
    let mut runtime = server.join().unwrap();
    let reworn = runtime
        .attached_terminal_wardrobe(&plan, &show, 1, TerminalWardrobeCommand::Wear)
        .unwrap();
    assert_eq!(reworn["wardrobe"]["revision"], 2);
    assert!(!reworn["selected"].is_null());
    assert!(reworn["show_id"].is_null());
    assert_eq!(reworn["fresh_show_required"], true);
    drop(client);
    retire_closed_attachment(&state, &mut runtime).unwrap();
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn wrong_token_cannot_attach_or_advertise_terminal_output() {
    let mut runtime = runtime();
    let state = state(&runtime);
    let before = runtime.host.advertisement().clone();
    let token = [7; 32];
    let request = request(&runtime, &[8; 32]);
    let (mut client, mut server) = UnixStream::pair().unwrap();
    let worker = thread::spawn(move || {
        wire::write_request(&mut client, &request).unwrap();
        match wire::read_reply(&mut client).unwrap() {
            AttachReply::Refused { code, .. } => assert_eq!(code, "unauthorized"),
            _ => panic!("wrong token must be refused"),
        }
    });
    let mut first = [0];
    server.read_exact(&mut first).unwrap();
    serve(&state, &mut server, &mut runtime, &token, first[0]).unwrap();
    worker.join().unwrap();
    assert!(!is_attached(&mut runtime));
    assert_eq!(runtime.host.advertisement(), &before);
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn stale_offer_generation_cannot_attach_a_terminal() {
    let mut runtime = runtime();
    let state = state(&runtime);
    let before = runtime.host.advertisement().clone();
    let token = [7; 32];
    let mut request = request(&runtime, &token);
    request.offer_generation.0 += 1;
    let (mut client, mut server) = UnixStream::pair().unwrap();
    let worker = thread::spawn(move || {
        wire::write_request(&mut client, &request).unwrap();
        match wire::read_reply(&mut client).unwrap() {
            AttachReply::Refused { code, .. } => {
                assert_eq!(code, "stale-terminal-attachment-basis")
            }
            _ => panic!("stale Host offer must be refused"),
        }
    });
    let mut first = [0];
    server.read_exact(&mut first).unwrap();
    serve(&state, &mut server, &mut runtime, &token, first[0]).unwrap();
    worker.join().unwrap();
    assert!(!is_attached(&mut runtime));
    assert_eq!(runtime.host.advertisement(), &before);
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn closed_provider_before_ack_never_retains_an_available_show() {
    let mut runtime = runtime();
    let state = state(&runtime);
    let token = [7; 32];
    let before = runtime.host.advertisement().clone();
    let request = request(&runtime, &token);
    let (mut client, mut server) = UnixStream::pair().unwrap();
    let worker = thread::spawn(move || {
        wire::write_request(&mut client, &request).unwrap();
        assert!(matches!(
            wire::read_reply(&mut client).unwrap(),
            AttachReply::Attached { protocol: PROTOCOL }
        ));
        // No foreground write or flush acknowledgement follows this close.
        drop(client);
    });
    let mut first = [0];
    server.read_exact(&mut first).unwrap();
    serve(&state, &mut server, &mut runtime, &token, first[0]).unwrap();
    worker.join().unwrap();
    assert!(!is_attached(&mut runtime));
    assert!(runtime.terminal_route.is_none());
    assert!(runtime.host.advertisement().offer_generation > before.offer_generation);
    fs::remove_dir_all(state).unwrap();
}
