use conduit_core::{
    bind_active_play, BaseImplementationId, BaseInstanceId, BootId, ConnectionId, FragmentId,
    HostId, KindId, LinkBindingId, LinkEndpointId, PlanId, PROTOCOL_VERSION,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, LineAttachment, SessionBinding,
    SessionEndpointIdentity, SessionLimits, SessionMachine, SessionMessage, SessionRole,
};

fn binding() -> SessionBinding {
    let plan_id = PlanId::from("bluetooth/capstone-plan-a");
    let source_host = HostId::from("bluetooth/std-host");
    let source_boot = BootId::from("bluetooth/std-boot-a");
    let sink_host = HostId::from("bluetooth/peer-host");
    let sink_boot = BootId::from("bluetooth/peer-boot-a");
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        source_active_play_id: bind_active_play(&plan_id, &source_host, &source_boot, 0)
            .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink_host, &sink_boot, 0).active_play_id,
        plan_id,
        source_fragment_id: FragmentId::from("bluetooth/source-fragment"),
        sink_fragment_id: FragmentId::from("bluetooth/sink-fragment"),
        connection_id: ConnectionId::from("unchanged/signal-cord"),
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
        attachment: LineAttachment {
            line_id: "bluetooth/line/a".into(),
            link_binding_id: LinkBindingId::from("bluetooth/binding/a"),
            base: BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1"),
            contract: conduit_bluetooth::BleGattProfile::line_contract(),
            base_instance_id: BaseInstanceId::from("bluez/hci0/session-a"),
            source_host_id: source_host,
            source_boot_id: source_boot,
            source_endpoint_id: LinkEndpointId::from("bluetooth/source-write"),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from("bluetooth/sink-indicate"),
            limits: conduit_bluetooth::BleGattProfile::FIRST
                .link_limits()
                .unwrap(),
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
fn ordinary_plan_scoped_session_frames_cross_the_ble_base_contract() {
    let binding = binding();
    let mut source = SessionMachine::new(binding.clone(), SessionRole::Source).unwrap();
    let mut sink = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
    activate(&mut source);
    activate(&mut sink);

    let offered = binding.frame(SessionMessage::Offered {
        sequence: 0,
        payload: b"signal-0",
    });
    let mut encoded = [0_u8; 2_048];
    let length = encode_session_frame_into(offered, &mut encoded, 96, 2_048).unwrap();
    let decoded = decode_session_frame(&encoded[..length], 96, 2_048).unwrap();
    source.admit_outbound(decoded).unwrap();
    sink.admit_inbound(decoded).unwrap();

    let accepted = binding.frame(SessionMessage::Accepted { sequence: 0 });
    sink.admit_outbound(accepted).unwrap();
    source.admit_inbound(accepted).unwrap();
    let delivered = binding.frame(SessionMessage::Delivered { sequence: 0 });
    sink.admit_outbound(delivered).unwrap();
    source.admit_inbound(delivered).unwrap();

    assert_eq!(
        source.binding().connection_id.as_str(),
        "unchanged/signal-cord"
    );
    assert_eq!(
        source.binding().attachment.base,
        BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1")
    );
    assert_eq!(source.next_sequence(), 1);
    assert_eq!(sink.next_sequence(), 1);
}

#[test]
fn stale_boot_and_base_epoch_frames_fail_at_the_shared_session_fence() {
    let expected = binding();
    let mut sink = SessionMachine::new(expected.clone(), SessionRole::Sink).unwrap();
    let mut stale_boot = expected.clone();
    stale_boot.source.boot_id = BootId::from("bluetooth/std-boot-stale");
    stale_boot.attachment.source_boot_id = stale_boot.source.boot_id.clone();
    assert_eq!(
        sink.admit_inbound(stale_boot.hello_frame()),
        Err(conduit_wire::WireError::BootMismatch)
    );

    let mut stale_epoch = expected.clone();
    stale_epoch.attachment.base_instance_id = BaseInstanceId::from("bluez/hci0/session-stale");
    assert_eq!(
        sink.admit_inbound(stale_epoch.hello_frame()),
        Err(conduit_wire::WireError::SessionEpochMismatch)
    );
}
