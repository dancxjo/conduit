use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
    HostId, KindId, LinkBindingId, LinkEndpointId, LinkLimits, PlanId, PROTOCOL_VERSION,
};
use conduit_wire::{
    decode_session_checkpoint, encode_session_checkpoint_into, RouteAttachment, SessionBinding,
    SessionEndpointIdentity, SessionLimits, SessionMachine, SessionMessage, SessionResumeAction,
    SessionRole, WireError,
};

fn binding(base: ConnectionBase) -> SessionBinding {
    let plan = PlanId::from("r1/plan-c");
    let source_host = HostId::from("r1/std-host");
    let source_boot = BootId::from("r1/std-boot");
    let sink_host = HostId::from("r1/pico-host");
    let sink_boot = BootId::from("r1/pico-boot");
    let suffix = match base {
        ConnectionBase::WebSocket => "websocket",
        ConnectionBase::UsbCdc => "usb-cdc",
        _ => unreachable!(),
    };
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        source_active_play_id: bind_active_play(&plan, &source_host, &source_boot, 0)
            .active_play_id,
        sink_active_play_id: bind_active_play(&plan, &sink_host, &sink_boot, 0).active_play_id,
        plan_id: plan,
        source_fragment_id: FragmentId::from("r1/source-fragment"),
        sink_fragment_id: FragmentId::from("r1/sink-fragment"),
        connection_id: ConnectionId::from("r1/signal-cord"),
        source: SessionEndpointIdentity {
            host_id: source_host.clone(),
            boot_id: source_boot.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink_host.clone(),
            boot_id: sink_boot.clone(),
        },
        value_kind: KindId::from("conduit.signal/level@1"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 9,
            maximum_buffered_bytes: 9,
        },
        attachment: RouteAttachment {
            link_binding_id: LinkBindingId::from(format!("r1/{suffix}-line")),
            base,
            base_instance_id: ConnectionBaseInstanceId::from(format!("r1/{suffix}-base")),
            source_host_id: source_host,
            source_boot_id: source_boot,
            source_endpoint_id: LinkEndpointId::from(format!("r1/{suffix}-source")),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from(format!("r1/{suffix}-sink")),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 9,
                maximum_buffered_bytes: 9,
                maximum_frame_bytes: 1024,
            },
        },
    }
}

fn activate(machine: &mut SessionMachine) {
    let binding = machine.binding().clone();
    machine.admit_outbound(binding.hello_frame()).unwrap();
    machine.admit_inbound(binding.hello_frame()).unwrap();
    machine
        .admit_outbound(binding.frame(SessionMessage::Ready))
        .unwrap();
    machine
        .admit_inbound(binding.frame(SessionMessage::Ready))
        .unwrap();
}

#[test]
fn peers_exchange_exact_bounded_checkpoints_before_same_plan_attachment_resume() {
    let websocket = binding(ConnectionBase::WebSocket);
    let usb = binding(ConnectionBase::UsbCdc);
    let mut source = SessionMachine::new(websocket.clone(), SessionRole::Source).unwrap();
    let mut sink = SessionMachine::new(websocket.clone(), SessionRole::Sink).unwrap();
    activate(&mut source);
    activate(&mut sink);

    let offered = websocket.frame(SessionMessage::Offered {
        sequence: 0,
        payload: &[1],
    });
    source.admit_outbound(offered).unwrap();
    sink.admit_inbound(offered).unwrap();

    let mut source_bytes = [0_u8; 1024];
    let source_len =
        encode_session_checkpoint_into(source.checkpoint_offer(), &mut source_bytes, 1024).unwrap();
    let mut sink_bytes = [0_u8; 1024];
    let sink_len =
        encode_session_checkpoint_into(sink.checkpoint_offer(), &mut sink_bytes, 1024).unwrap();

    let source_peer = decode_session_checkpoint(&sink_bytes[..sink_len], 1024).unwrap();
    let sink_peer = decode_session_checkpoint(&source_bytes[..source_len], 1024).unwrap();
    let source_acceptance = source
        .resume_with_attachment(usb.clone(), source_peer)
        .unwrap();
    let sink_acceptance = sink.resume_with_attachment(usb, sink_peer).unwrap();

    assert_eq!(source_acceptance.action, SessionResumeAction::Continue);
    assert_eq!(sink_acceptance.action, SessionResumeAction::Continue);
    assert!(source_acceptance.same_plan_continues);
    assert!(sink_acceptance.same_plan_continues);
    assert_eq!(source.binding().plan_id.as_str(), "r1/plan-c");
    assert_eq!(sink.binding().plan_id.as_str(), "r1/plan-c");
    assert_eq!(source.binding().attachment.base, ConnectionBase::UsbCdc);
    assert_eq!(sink.binding().attachment.base, ConnectionBase::UsbCdc);
}

#[test]
fn checkpoint_for_another_logical_session_is_rejected_after_wire_decode() {
    let websocket = binding(ConnectionBase::WebSocket);
    let usb = binding(ConnectionBase::UsbCdc);
    let mut source = SessionMachine::new(websocket, SessionRole::Source).unwrap();
    activate(&mut source);
    let mut wrong_binding = usb.clone();
    wrong_binding.connection_id = ConnectionId::from("r1/other-cord");
    let mut wrong = SessionMachine::new(wrong_binding, SessionRole::Sink).unwrap();
    activate(&mut wrong);
    let mut bytes = [0_u8; 1024];
    let length =
        encode_session_checkpoint_into(wrong.checkpoint_offer(), &mut bytes, 1024).unwrap();
    let decoded = decode_session_checkpoint(&bytes[..length], 1024).unwrap();

    assert_eq!(
        source.resume_with_attachment(usb, decoded),
        Err(WireError::InvalidSession)
    );
}
