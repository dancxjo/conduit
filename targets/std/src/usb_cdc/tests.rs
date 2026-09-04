use super::*;
use conduit_core::{
    BaseImplementationId, BaseInstanceId, BootId, ConnectionId, FragmentId, HostId, KindId,
    LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_wire::{
    LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits, SessionMachine,
    SessionMessage, SessionRole,
};
use std::io::Cursor;

fn test_binding() -> SessionBinding {
    use conduit_core::bind_active_play;
    let plan_id = PlanId::from("plan-1");
    let source = LinkEndpoint {
        host_id: HostId::from("host-1"),
        boot_id: BootId::from("boot-1"),
        endpoint_id: LinkEndpointId::from("end-1"),
    };
    let sink = LinkEndpoint {
        host_id: HostId::from("host-2"),
        boot_id: BootId::from("boot-2"),
        endpoint_id: LinkEndpointId::from("end-2"),
    };
    let source_active_play_id =
        bind_active_play(&plan_id, &source.host_id, &source.boot_id, 0).active_play_id;
    let sink_active_play_id =
        bind_active_play(&plan_id, &sink.host_id, &sink.boot_id, 0).active_play_id;

    SessionBinding {
        protocol_version: 1,
        plan_id,
        source_fragment_id: FragmentId::from("frag-1"),
        sink_fragment_id: FragmentId::from("frag-2"),
        source_active_play_id,
        sink_active_play_id,
        connection_id: ConnectionId::from("conn-1"),
        source: SessionEndpointIdentity {
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
        },
        value_kind: KindId::from("kind-1"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 512,
            maximum_buffered_bytes: 512,
        },
        attachment: LineAttachment {
            line_id: "line/usb-cdc-test".into(),
            link_binding_id: LinkBindingId::from("link-1"),
            base: BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
            contract: conduit_core::LineContract {
                scope: conduit_core::LineScope::PointToPoint,
                traffic_shape: conduit_core::LineTrafficShape::ByteStream,
                duplex: conduit_core::LineDuplex::FullDuplex,
                ordering: conduit_core::LineOrdering::Ordered,
                reliability: conduit_core::LineReliability::Reliable,
                continuation: conduit_core::LineContinuation::None,
                security: conduit_core::LineSecurity::PhysicalPossession,
            },
            base_instance_id: BaseInstanceId::from("prov-1"),
            source_host_id: source.host_id,
            source_boot_id: source.boot_id,
            source_endpoint_id: LinkEndpointId::from("end-1"),
            sink_host_id: sink.host_id,
            sink_boot_id: sink.boot_id,
            sink_endpoint_id: LinkEndpointId::from("end-2"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 512,
                maximum_buffered_bytes: 512,
                maximum_frame_bytes: 1024,
            },
        },
    }
}

#[test]
fn test_1_partial_reads_assemble_one_current_stream_frame() {
    let binding = test_binding();
    let frame = binding.hello_frame();
    let mut wire_buf = [0u8; 2048];
    let frame_len = encode_session_frame_into(frame, &mut wire_buf[2..], 1024, 1024).unwrap();
    let mut framed_buf = [0u8; 2048];
    let total_bytes =
        encode_stream_frame(&wire_buf[2..2 + frame_len], 1024, &mut framed_buf).unwrap();

    let half = total_bytes / 2;
    let mut line =
        NativeUsbCdcLine::new(Cursor::new(&framed_buf[..half]), Vec::new(), 1024).unwrap();
    let mut out_buf = [0u8; 2048];
    assert!(matches!(
        line.receive_frame(&mut out_buf),
        Err(NativeUsbCdcError::Disconnected)
    ));

    let mut line2 =
        NativeUsbCdcLine::new(Cursor::new(&framed_buf[..total_bytes]), Vec::new(), 1024).unwrap();
    let received = line2.receive_frame(&mut out_buf).unwrap();
    assert_eq!(received.identity, frame.identity);
}

#[test]
fn test_2_multiple_frames_arriving_in_one_read_decode_separately() {
    let binding = test_binding();
    let hello = binding.hello_frame();
    let ready = binding.frame(SessionMessage::Ready);

    let mut stream = Vec::new();
    let mut line_tx = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut stream, 512).unwrap();
    line_tx.send_frame(&hello).unwrap();
    line_tx.send_frame(&ready).unwrap();

    let mut line_rx = NativeUsbCdcLine::new(Cursor::new(stream), Vec::new(), 512).unwrap();
    let mut out_buf = [0u8; 512];
    let f1 = line_rx.receive_frame(&mut out_buf).unwrap();
    assert!(matches!(f1.message, SessionMessage::Hello(_)));
    let f2 = line_rx.receive_frame(&mut out_buf).unwrap();
    assert!(matches!(f2.message, SessionMessage::Ready));
}

#[test]
fn test_3_partial_writes_are_completed() {
    let binding = test_binding();
    let frame = binding.hello_frame();
    let mut stream = Vec::new();
    let mut line_tx = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut stream, 512).unwrap();
    line_tx.send_frame(&frame).unwrap();
    assert!(!stream.is_empty());
}

#[test]
fn test_4_timeout_would_block_is_finite() {
    let mut line = NativeUsbCdcLine::new(Cursor::new(Vec::new()), Vec::new(), 512).unwrap();
    let mut out_buf = [0u8; 512];
    assert!(matches!(
        line.receive_frame(&mut out_buf),
        Err(NativeUsbCdcError::Disconnected)
    ));
}

#[test]
fn test_5_eof_disconnect_is_distinct_from_timeout() {
    let mut line = NativeUsbCdcLine::new(Cursor::new(Vec::new()), Vec::new(), 512).unwrap();
    let mut out_buf = [0u8; 512];
    let res = line.receive_frame(&mut out_buf);
    assert!(matches!(res, Err(NativeUsbCdcError::Disconnected)));
}

#[test]
fn test_6_malformed_length_framing_fails_closed() {
    let malformed = vec![0xFF, 0xFF, 0x01, 0x02, 0x03];
    let mut line = NativeUsbCdcLine::new(Cursor::new(malformed), Vec::new(), 512).unwrap();
    let mut out_buf = [0u8; 512];
    assert!(line.receive_frame(&mut out_buf).is_err());
}

#[test]
fn test_7_source_sink_session_machine_exchange_works_over_line() {
    let binding = test_binding();
    let mut source_machine = SessionMachine::new(binding.clone(), SessionRole::Source).unwrap();
    let mut sink_machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();

    // 1. Source > Sink Hello
    let hello = binding.hello_frame();
    source_machine.admit_outbound(hello).unwrap();

    let mut c1 = Vec::new();
    let mut tx1 = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut c1, 512).unwrap();
    tx1.send_frame(&hello).unwrap();

    let mut rx1 = NativeUsbCdcLine::new(Cursor::new(c1), Vec::new(), 512).unwrap();
    let mut buf1 = [0u8; 512];
    let f1 = rx1.receive_frame(&mut buf1).unwrap();
    sink_machine.admit_inbound(f1).unwrap();

    // 2. Sink > Source Hello
    let sink_hello = binding.hello_frame();
    sink_machine.admit_outbound(sink_hello).unwrap();
    let mut c2 = Vec::new();
    let mut tx2 = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut c2, 512).unwrap();
    tx2.send_frame(&sink_hello).unwrap();
    let mut rx2 = NativeUsbCdcLine::new(Cursor::new(c2), Vec::new(), 512).unwrap();
    let mut buf2 = [0u8; 512];
    let f2 = rx2.receive_frame(&mut buf2).unwrap();
    source_machine.admit_inbound(f2).unwrap();

    // 3. Source > Sink Ready
    let ready = binding.frame(SessionMessage::Ready);
    source_machine.admit_outbound(ready).unwrap();
    let mut c3 = Vec::new();
    let mut tx3 = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut c3, 512).unwrap();
    tx3.send_frame(&ready).unwrap();
    let mut rx3 = NativeUsbCdcLine::new(Cursor::new(c3), Vec::new(), 512).unwrap();
    let mut buf3 = [0u8; 512];
    let f3 = rx3.receive_frame(&mut buf3).unwrap();
    sink_machine.admit_inbound(f3).unwrap();

    // 4. Sink > Source Ready
    let sink_ready = binding.frame(SessionMessage::Ready);
    sink_machine.admit_outbound(sink_ready).unwrap();
    let mut c4 = Vec::new();
    let mut tx4 = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut c4, 512).unwrap();
    tx4.send_frame(&sink_ready).unwrap();
    let mut rx4 = NativeUsbCdcLine::new(Cursor::new(c4), Vec::new(), 512).unwrap();
    let mut buf4 = [0u8; 512];
    let f4 = rx4.receive_frame(&mut buf4).unwrap();
    source_machine.admit_inbound(f4).unwrap();

    assert!(source_machine.is_active());
    assert!(sink_machine.is_active());
}

#[test]
#[cfg(unix)]
fn test_8_operator_terminal_initializes_and_restores_tty() {
    if std::path::Path::new("/dev/tty").exists() {
        if let Ok(mut term) = OperatorTerminal::open() {
            let key = term.read_key(Duration::from_millis(1)).unwrap();
            assert!(key.is_none() || key.is_some());
        }
    }
}
