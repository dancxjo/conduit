use super::*;
use crate::durable_host::owner::Owner;
use conduit_body::ResidentPlot;
use conduit_std_host::{hosted_terminal_mask_host::receive_terminal_frame_and_ack, StdHost};
use std::{io::Read, path::PathBuf, thread};

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

#[test]
fn current_lulled_owner_attaches_actual_host_and_acknowledges_one_read_only_show() {
    let mut runtime = runtime();
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
    serve(&mut server, &mut runtime, &token, first[0]).unwrap();
    let (client, route_plan_id, show) = worker.join().unwrap();
    assert!(is_attached(&mut runtime));
    let route = runtime.terminal_route.as_ref().unwrap();
    assert_eq!(route.seal.route_plan_id, route_plan_id);
    assert_eq!(route.show, show);
    assert!(matches!(
        route.show.show.lifecycle,
        conduit_presentation::ManifestationLifecycle::Available
    ));
    drop(client);
    retire_closed_attachment(&mut runtime).unwrap();
    assert!(!is_attached(&mut runtime));
    assert!(runtime.terminal_route.is_none());
    assert!(runtime.host.advertisement().offer_generation > before.offer_generation);
}

#[test]
fn wrong_token_cannot_attach_or_advertise_terminal_output() {
    let mut runtime = runtime();
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
    serve(&mut server, &mut runtime, &token, first[0]).unwrap();
    worker.join().unwrap();
    assert!(!is_attached(&mut runtime));
    assert_eq!(runtime.host.advertisement(), &before);
}

#[test]
fn stale_offer_generation_cannot_attach_a_terminal() {
    let mut runtime = runtime();
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
    serve(&mut server, &mut runtime, &token, first[0]).unwrap();
    worker.join().unwrap();
    assert!(!is_attached(&mut runtime));
    assert_eq!(runtime.host.advertisement(), &before);
}

#[test]
fn closed_provider_before_ack_never_retains_an_available_show() {
    let mut runtime = runtime();
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
    serve(&mut server, &mut runtime, &token, first[0]).unwrap();
    worker.join().unwrap();
    assert!(!is_attached(&mut runtime));
    assert!(runtime.terminal_route.is_none());
    assert!(runtime.host.advertisement().offer_generation > before.offer_generation);
}
