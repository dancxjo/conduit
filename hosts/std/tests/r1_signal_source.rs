use conduit_core::{BootId, ConnectionBase, HostId, LinkAvailability, LinkBinding};
use conduit_std_host::pico_usb_source::PicoUsbSource;
use conduit_system_continuity::{exact_r1_signal_plan, R1SignalRouteSet};
use conduit_wire::{
    decode_session_checkpoint, encode_session_checkpoint_into, SessionMachine, SessionMessage,
    SessionResumeAction, SessionRole,
};

#[test]
fn one_production_source_executes_both_exact_r1_recovery_plans() {
    let pico_boot = BootId::from("r1/test-pico-runtime-boot");
    let source_host = HostId::from(conduit_net::R1_STD_HOST_ID);
    let plan_a = exact_r1_signal_plan(pico_boot.clone(), R1SignalRouteSet::WebSocketOnly)
        .expect("WebSocket-only Plan A");
    let plan_b =
        exact_r1_signal_plan(pico_boot, R1SignalRouteSet::UsbOnly).expect("USB-only Plan B");

    let source_a = PicoUsbSource::prepare_plan(plan_a.plan, &source_host)
        .expect("production source prepares Plan A");
    let source_b = PicoUsbSource::prepare_plan(plan_b.plan, &source_host)
        .expect("production source prepares Plan B");

    assert_eq!(
        source_a.binding().attachment.base,
        ConnectionBase::WebSocket
    );
    assert_eq!(source_b.binding().attachment.base, ConnectionBase::UsbCdc);
    assert_eq!(source_a.source_host_id(), source_b.source_host_id());
    assert_eq!(source_a.binding().sink, source_b.binding().sink);
    assert_ne!(source_a.binding().plan_id, source_b.binding().plan_id);
    assert_ne!(
        source_a.binding().source_active_play_id,
        source_b.binding().source_active_play_id
    );
    assert_ne!(
        source_a.binding().sink_active_play_id,
        source_b.binding().sink_active_play_id
    );
}

#[test]
fn production_source_retains_plan_play_and_offer_across_sealed_usb_resume() {
    let exact = exact_r1_signal_plan(
        BootId::from(conduit_net::R1_PICO_BOOT_ID),
        R1SignalRouteSet::WebSocketThenUsb,
    )
    .expect("dual-Line Plan C");
    let connection = exact
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|connection| connection.route_candidates.len() == 2)
        .expect("dual-Line Cord")
        .clone();
    let usb = &connection.route_candidates[1];
    let usb = LinkBinding {
        binding_id: usb.binding_id.clone(),
        source: usb.source.clone(),
        sink: usb.sink.clone(),
        base: usb.base,
        base_instance_id: usb.base_instance_id.clone(),
        availability: LinkAvailability::Ready,
        credential: usb.credential.clone(),
        authority: usb.authority.clone(),
        limits: usb.limits,
    };
    let mut source =
        PicoUsbSource::prepare_plan(exact.plan, &HostId::from(conduit_net::R1_STD_HOST_ID))
            .expect("production Plan-C source");
    source
        .observe_sink_boot(BootId::from("r1/runtime-pico-boot"))
        .unwrap();
    let websocket = source.binding().clone();
    let mut sink = SessionMachine::new(websocket.clone(), SessionRole::Sink).unwrap();
    let hello = websocket.hello_frame();
    source.admit_outbound(hello).unwrap();
    sink.admit_inbound(hello).unwrap();
    sink.admit_outbound(hello).unwrap();
    source.admit_inbound(hello).unwrap();
    let ready = websocket.frame(SessionMessage::Ready);
    source.admit_outbound(ready).unwrap();
    sink.admit_inbound(ready).unwrap();
    sink.admit_outbound(ready).unwrap();
    source.admit_inbound(ready).unwrap();

    let (sequence, payload) = source.next_offer().unwrap().expect("first offer");
    let offered = websocket.frame(SessionMessage::Offered {
        sequence,
        payload: &payload,
    });
    source.admit_outbound(offered).unwrap();
    // The WebSocket becomes unavailable before the sink observes this offer.

    let mut source_checkpoint = [0_u8; 1024];
    let source_checkpoint_len =
        encode_session_checkpoint_into(source.checkpoint_offer(), &mut source_checkpoint, 1024)
            .unwrap();
    let plan_id = source.binding().plan_id.clone();
    let source_play_id = source.binding().source_active_play_id.clone();
    let sink_play_id = source.binding().sink_active_play_id.clone();
    let usb_binding = conduit_wire::SessionBinding::from_planned_connection_with_link(
        source.binding().plan_id.clone(),
        source.binding().source_fragment_id.clone(),
        source.binding().sink_fragment_id.clone(),
        &connection,
        &usb,
    )
    .unwrap()
    .with_observed_boots(
        source.binding().source.boot_id.clone(),
        source.binding().sink.boot_id.clone(),
    )
    .unwrap();
    let source_acceptance = source
        .resume_with_link(&usb, sink.checkpoint_offer())
        .unwrap();
    let source_offer =
        decode_session_checkpoint(&source_checkpoint[..source_checkpoint_len], 1024).unwrap();
    let sink_acceptance = sink
        .resume_with_attachment(usb_binding, source_offer)
        .unwrap();

    assert_eq!(
        source_acceptance.action,
        SessionResumeAction::ReplayOffered(0)
    );
    assert_eq!(sink_acceptance.action, SessionResumeAction::AwaitReplay(0));
    assert_eq!(source.binding().plan_id, plan_id);
    assert_eq!(source.binding().source_active_play_id, source_play_id);
    assert_eq!(source.binding().sink_active_play_id, sink_play_id);
    assert_eq!(source.binding().attachment.base, ConnectionBase::UsbCdc);
    assert_eq!(source.next_offer().unwrap().unwrap().0, 0);
}
