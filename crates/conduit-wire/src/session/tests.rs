use super::*;
use alloc::vec;
use conduit_core::{
    AdmittedLine, BootId, BoundLink, HostId, LineContinuation, LineContract, LineDuplex, LineId,
    LineOrdering, LineReliability, LineScope, LineSecurity, LineTrafficShape,
    LinkAuthorityReference, LinkCredentialReference, LinkEndpoint, LinkEndpointId, PlacementId,
};

const MAXIMUM_PAYLOAD_BYTES: u32 = 16;
const MAXIMUM_FRAME_BYTES: u32 = 512;

fn remote_session_contract() -> LineContract {
    LineContract {
        scope: LineScope::LocalNetwork,
        traffic_shape: LineTrafficShape::Message,
        duplex: LineDuplex::FullDuplex,
        ordering: LineOrdering::Ordered,
        reliability: LineReliability::Reliable,
        continuation: LineContinuation::None,
        security: LineSecurity::PlaintextNetwork,
    }
}

fn binding() -> SessionBinding {
    let plan_id = PlanId::from("test/plan");
    let source = LinkEndpoint {
        host_id: HostId::from("test/source-host"),
        boot_id: BootId::from("test/source-boot"),
        endpoint_id: LinkEndpointId::from("test/source-endpoint"),
    };
    let sink = LinkEndpoint {
        host_id: HostId::from("test/sink-host"),
        boot_id: BootId::from("test/sink-boot"),
        endpoint_id: LinkEndpointId::from("test/sink-endpoint"),
    };
    let source_active_play_id =
        bind_active_play(&plan_id, &source.host_id, &source.boot_id, 0).active_play_id;
    let sink_active_play_id =
        bind_active_play(&plan_id, &sink.host_id, &sink.boot_id, 0).active_play_id;
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        plan_id,
        source_fragment_id: FragmentId::from("test/source-fragment"),
        sink_fragment_id: FragmentId::from("test/sink-fragment"),
        source_active_play_id,
        sink_active_play_id,
        connection_id: ConnectionId::from("test/connection"),
        source: SessionEndpointIdentity {
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
        },
        value_kind: KindId::from("test/value"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: MAXIMUM_PAYLOAD_BYTES,
            maximum_buffered_bytes: MAXIMUM_PAYLOAD_BYTES,
        },
        attachment: LineAttachment {
            line_id: LineId::from("test/line"),
            link_binding_id: LinkBindingId::from("test/link"),
            base: BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            contract: remote_session_contract(),
            base_instance_id: BaseInstanceId::from("test/base-instance"),
            source_host_id: source.host_id,
            source_boot_id: source.boot_id,
            source_endpoint_id: LinkEndpointId::from("test/source-endpoint"),
            sink_host_id: sink.host_id,
            sink_boot_id: sink.boot_id,
            sink_endpoint_id: LinkEndpointId::from("test/sink-endpoint"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: MAXIMUM_PAYLOAD_BYTES,
                maximum_buffered_bytes: MAXIMUM_PAYLOAD_BYTES,
                maximum_frame_bytes: MAXIMUM_FRAME_BYTES,
            },
        },
    }
}

fn planned_connection(expected: &SessionBinding, base: BaseImplementationId) -> PlannedConnection {
    let line = AdmittedLine {
        line_id: expected.attachment.line_id.clone(),
        binding: BoundLink {
            binding_id: expected.attachment.link_binding_id.clone(),
            source: LinkEndpoint {
                host_id: expected.source.host_id.clone(),
                boot_id: expected.source.boot_id.clone(),
                endpoint_id: expected.attachment.source_endpoint_id.clone(),
            },
            sink: LinkEndpoint {
                host_id: expected.sink.host_id.clone(),
                boot_id: expected.sink.boot_id.clone(),
                endpoint_id: expected.attachment.sink_endpoint_id.clone(),
            },
            base,
            base_instance_id: expected.attachment.base_instance_id.clone(),
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: expected.attachment.limits,
        },
        contract: remote_session_contract(),
    };
    PlannedConnection {
        connection_id: expected.connection_id.clone(),
        source_placement_id: PlacementId::from("test/source-placement"),
        source_port_id: conduit_core::PortId::from("out"),
        sink_placement_id: PlacementId::from("test/sink-placement"),
        sink_port_id: conduit_core::PortId::from("in"),
        value_kind: expected.value_kind.clone(),
        temporal: conduit_core::PortTemporal::Value,
        selected_line: Some(line.clone()),
        admitted_lines: vec![line],
        item_capacity: 1,
        byte_capacity: MAXIMUM_PAYLOAD_BYTES,
    }
}

fn trigger(machine: &mut SessionMachine) {
    let binding = machine.binding().clone();
    machine.admit_outbound(binding.hello_frame()).unwrap();
    machine.admit_inbound(binding.hello_frame()).unwrap();
    machine
        .admit_outbound(binding.frame(SessionMessage::Ready))
        .unwrap();
    machine
        .admit_inbound(binding.frame(SessionMessage::Ready))
        .unwrap();
    assert!(machine.is_active());
}

#[test]
fn exact_planned_connection_constructs_the_session_binding() {
    let expected = binding();
    let connection = planned_connection(
        &expected,
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
    );
    let actual = SessionBinding::from_planned_connection(
        expected.plan_id.clone(),
        expected.source_fragment_id.clone(),
        expected.sink_fragment_id.clone(),
        &connection,
    )
    .unwrap();
    assert_eq!(actual, expected);

    let mut missing = connection.clone();
    missing.selected_line = None;
    assert_eq!(
        SessionBinding::from_planned_connection(
            expected.plan_id,
            expected.source_fragment_id,
            expected.sink_fragment_id,
            &missing,
        ),
        Err(WireError::InvalidSession)
    );
}

#[test]
fn borrowed_codec_round_trips_exact_hello_and_rejects_frame_mutations() {
    let binding = binding();
    let mut output = [0_u8; MAXIMUM_FRAME_BYTES as usize];
    let length = encode_session_frame_into(
        binding.hello_frame(),
        &mut output,
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .unwrap();
    let decoded = decode_session_frame(
        &output[..length],
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .unwrap();
    assert_eq!(decoded, binding.hello_frame());
    assert_eq!(
        SessionBinding::from_hello_frame(decoded),
        Ok(binding.clone())
    );
    assert_eq!(
        SessionBinding::from_hello_frame(binding.frame(SessionMessage::Ready)),
        Err(WireError::InvalidSession)
    );
    let dynamic = binding
        .clone()
        .with_observed_boots(
            BootId::from("browser-boot/dynamic-source"),
            BootId::from("browser-boot/dynamic-sink"),
        )
        .unwrap();
    let mut dynamic_output = [0_u8; MAXIMUM_FRAME_BYTES as usize];
    let dynamic_length = encode_session_frame_into(
        dynamic.hello_frame(),
        &mut dynamic_output,
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .unwrap();
    let dynamic_frame = decode_session_frame(
        &dynamic_output[..dynamic_length],
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .unwrap();
    assert_eq!(SessionBinding::from_hello_frame(dynamic_frame), Ok(dynamic));
    assert_eq!(
        decode_session_frame(
            &output[..length - 1],
            MAXIMUM_PAYLOAD_BYTES,
            MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::TruncatedFrame)
    );

    output[length] = 0;
    assert_eq!(
        decode_session_frame(
            &output[..length + 1],
            MAXIMUM_PAYLOAD_BYTES,
            MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::TrailingGarbage)
    );
    assert_eq!(
        encode_session_frame_into(
            binding.hello_frame(),
            &mut [0_u8; 32],
            MAXIMUM_PAYLOAD_BYTES,
            MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::OutputTooSmall)
    );
}

#[test]
fn lifecycle_is_exact_bounded_and_terminal() {
    let binding = binding();
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Source).unwrap();
    trigger(&mut machine);

    let offered = binding.frame(SessionMessage::Offered {
        sequence: 0,
        payload: b"signal-0",
    });
    assert_eq!(
        machine.admit_outbound(binding.frame(SessionMessage::Offered {
            sequence: 1,
            payload: b"signal-1",
        })),
        Err(WireError::ReorderedFrame)
    );
    machine.admit_outbound(offered).unwrap();
    assert_eq!(
        machine.admit_outbound(offered),
        Err(WireError::DuplicateFrame)
    );
    machine
        .admit_inbound(binding.frame(SessionMessage::Accepted { sequence: 0 }))
        .unwrap();
    machine
        .admit_inbound(binding.frame(SessionMessage::Delivered { sequence: 0 }))
        .unwrap();
    assert_eq!(machine.next_sequence(), 1);
    assert_eq!(
        machine.admit_inbound(binding.frame(SessionMessage::Delivered { sequence: 0 })),
        Err(WireError::DuplicateFrame)
    );
    machine
        .admit_outbound(binding.frame(SessionMessage::InputClosed { final_sequence: 1 }))
        .unwrap();
    let terminal = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Completed,
        final_sequence: 1,
    });
    machine.admit_outbound(terminal).unwrap();
    machine.admit_inbound(terminal).unwrap();
    assert!(machine.is_terminal());
    assert_eq!(machine.admit_inbound(terminal), Err(WireError::LateFrame));
}

#[test]
fn receiver_pressure_allows_only_the_same_offer_to_retry() {
    let binding = binding();
    let mut source = SessionMachine::new(binding.clone(), SessionRole::Source).unwrap();
    trigger(&mut source);
    let offered = binding.frame(SessionMessage::Offered {
        sequence: 0,
        payload: b"signal-0",
    });
    source.admit_outbound(offered).unwrap();
    source
        .admit_inbound(binding.frame(SessionMessage::Pressure { sequence: 0 }))
        .unwrap();
    assert_eq!(source.next_sequence(), 0);
    source.admit_outbound(offered).unwrap();
    assert_eq!(
        source.admit_inbound(binding.frame(SessionMessage::Pressure { sequence: 1 })),
        Err(WireError::ReorderedFrame)
    );
    source
        .admit_inbound(binding.frame(SessionMessage::Accepted { sequence: 0 }))
        .unwrap();
    source
        .admit_inbound(binding.frame(SessionMessage::Delivered { sequence: 0 }))
        .unwrap();
    assert_eq!(source.next_sequence(), 1);
}

#[test]
fn cancellation_requires_matching_peer_fact_and_terminal_disposition() {
    let binding = binding();
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
    trigger(&mut machine);
    let cancelled = binding.frame(SessionMessage::Cancelled { code: 7 });
    machine.admit_outbound(cancelled).unwrap();
    assert_eq!(
        machine.admit_inbound(binding.frame(SessionMessage::Failed { code: 7 })),
        Err(WireError::InvalidState)
    );
    machine.admit_inbound(cancelled).unwrap();
    let terminal = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Cancelled,
        final_sequence: 0,
    });
    machine.admit_outbound(terminal).unwrap();
    machine.admit_inbound(terminal).unwrap();
    assert!(machine.is_terminal());
}

#[test]
fn wrong_identity_hello_and_limits_fail_closed() {
    let binding = binding();
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Source).unwrap();
    let mut wrong_identity = binding.identity();
    wrong_identity.plan_id = "other/plan";
    assert_eq!(
        machine.admit_inbound(SessionFrame {
            identity: wrong_identity,
            message: binding.hello_frame().message,
        }),
        Err(WireError::PlanMismatch)
    );

    let mut wrong_hello = match binding.hello_frame().message {
        SessionMessage::Hello(hello) => hello,
        _ => unreachable!(),
    };
    wrong_hello.limits.maximum_payload_bytes += 1;
    assert_eq!(
        machine.admit_inbound(binding.frame(SessionMessage::Hello(wrong_hello))),
        Err(WireError::InvalidLimits)
    );

    let mut invalid = binding;
    invalid.attachment.limits.maximum_frame_bytes = 32;
    assert!(matches!(
        SessionMachine::new(invalid, SessionRole::Source),
        Err(WireError::InvalidLimits)
    ));
}

#[test]
fn every_logical_identity_mutation_fails_closed() {
    let binding = binding();
    for field in 0..14 {
        let mut frame = binding.hello_frame();
        match field {
            0 => frame.identity.plan_id = "wrong/plan",
            1 => frame.identity.source_fragment_id = "wrong/source-fragment",
            2 => frame.identity.sink_fragment_id = "wrong/sink-fragment",
            3 => frame.identity.connection_id = "wrong/connection",
            4 => frame.identity.source_active_play_id = "wrong/source-play",
            5 => frame.identity.sink_active_play_id = "wrong/sink-play",
            6 => frame.identity.source_host_id = "wrong/source-host",
            7 => frame.identity.source_boot_id = "wrong/source-boot",
            8 => frame.identity.sink_host_id = "wrong/sink-host",
            9 => frame.identity.sink_boot_id = "wrong/sink-boot",
            10 => frame.identity.value_kind = "wrong/value-kind",
            11 => frame.identity.limits.maximum_in_flight_items += 1,
            12 => frame.identity.limits.maximum_payload_bytes += 1,
            13 => frame.identity.limits.maximum_buffered_bytes += 1,
            _ => unreachable!(),
        }
        let expected = match field {
            0 => WireError::PlanMismatch,
            7 | 9 => WireError::BootMismatch,
            3 => WireError::ConnectionMismatch,
            10 => WireError::ValueContractMismatch,
            11..=13 => WireError::InvalidLimits,
            _ => WireError::InvalidSession,
        };
        let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
        assert_eq!(machine.admit_inbound(frame), Err(expected));
    }
    let mut hello = match binding.hello_frame().message {
        SessionMessage::Hello(hello) => hello,
        _ => unreachable!(),
    };
    hello.link_binding_id = "wrong/link";
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
    assert_eq!(
        machine.admit_inbound(binding.frame(SessionMessage::Hello(hello))),
        Err(WireError::SessionEpochMismatch)
    );
}

#[test]
fn every_route_attachment_wire_fact_is_checked_separately() {
    let binding = binding();
    for field in 0..6 {
        let mut hello = match binding.hello_frame().message {
            SessionMessage::Hello(hello) => hello,
            _ => unreachable!(),
        };
        match field {
            0 => hello.link_binding_id = "wrong/link",
            1 => hello.base = "conduit.base/usb-cdc-acm@1",
            2 => hello.base_instance_id = "wrong/base-instance",
            3 => hello.source_endpoint_id = "wrong/source-endpoint",
            4 => hello.sink_endpoint_id = "wrong/sink-endpoint",
            5 => hello.limits.maximum_frame_bytes += 1,
            _ => unreachable!(),
        }
        let expected = match field {
            0 | 2 => WireError::SessionEpochMismatch,
            5 => WireError::InvalidLimits,
            _ => WireError::InvalidSession,
        };
        let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
        assert_eq!(
            machine.admit_inbound(binding.frame(SessionMessage::Hello(hello))),
            Err(expected)
        );
    }
}

#[test]
fn session_codec_rejects_malformed_truncated_oversized_and_trailing_frames() {
    let binding = binding();
    let mut output = [0_u8; MAXIMUM_FRAME_BYTES as usize];
    let length = encode_session_frame_into(
        binding.frame(SessionMessage::Offered {
            sequence: 0,
            payload: b"signal-0",
        }),
        &mut output,
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .unwrap();
    let mut malformed = output[..length].to_vec();
    malformed[0] ^= 0xff;
    assert_eq!(
        decode_session_frame(&malformed, MAXIMUM_PAYLOAD_BYTES, MAXIMUM_FRAME_BYTES,),
        Err(WireError::InvalidMagic)
    );
    assert_eq!(
        decode_session_frame(
            &output[..length - 1],
            MAXIMUM_PAYLOAD_BYTES,
            MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::TruncatedFrame)
    );
    assert_eq!(
        decode_session_frame(
            &output[..length],
            MAXIMUM_PAYLOAD_BYTES,
            (length - 1) as u32,
        ),
        Err(WireError::OversizedFrame)
    );
    output[length] = 0;
    assert_eq!(
        decode_session_frame(
            &output[..length + 1],
            MAXIMUM_PAYLOAD_BYTES,
            MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::TrailingGarbage)
    );
}

#[test]
fn fixture_frame_exercises_remote_session_contract_without_transport_claim() {
    // Contract-level proof that FixtureFrame exercises the exact framed remote session
    // protocol. This proves session line neutrality without claiming that FixtureFrame
    // is an installed, runnable, or production-ready physical transport.
    let mut expected = binding();
    expected.attachment.base = BaseImplementationId::from("conduit.proof/frame@1");
    let connection = planned_connection(
        &expected,
        BaseImplementationId::from("conduit.proof/frame@1"),
    );

    let actual = SessionBinding::from_planned_connection(
        expected.plan_id.clone(),
        expected.source_fragment_id.clone(),
        expected.sink_fragment_id.clone(),
        &connection,
    )
    .unwrap();
    assert_eq!(actual, expected);
    actual.validate().unwrap();

    let mut output = [0_u8; MAXIMUM_FRAME_BYTES as usize];
    let length = encode_session_frame_into(
        actual.hello_frame(),
        &mut output,
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .unwrap();
    let decoded = decode_session_frame(
        &output[..length],
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .unwrap();
    assert_eq!(decoded, actual.hello_frame());

    let mut machine = SessionMachine::new(actual.clone(), SessionRole::Source).unwrap();
    trigger(&mut machine);
    assert!(machine.is_active());
}

#[test]
fn base_identity_cannot_override_an_ineligible_line_contract() {
    let binding = binding();
    for connection_base in [
        BaseImplementationId::from("conduit.base/local@1"),
        BaseImplementationId::from("third.party.base/quic@1"),
    ] {
        let mut invalid = binding.clone();
        invalid.attachment.base = connection_base.clone();
        invalid.attachment.contract.scope = LineScope::Process;
        assert_eq!(invalid.validate(), Err(WireError::InvalidBase));

        let mut connection = planned_connection(&binding, connection_base);
        connection.selected_line.as_mut().unwrap().contract.scope = LineScope::Process;
        connection.admitted_lines[0].contract.scope = LineScope::Process;
        assert_eq!(
            SessionBinding::from_planned_connection(
                binding.plan_id.clone(),
                binding.source_fragment_id.clone(),
                binding.sink_fragment_id.clone(),
                &connection,
            ),
            Err(WireError::InvalidSession)
        );
    }
}

#[test]
fn datagram_contract_remains_rejected_independently_of_base_identity() {
    let base = BaseImplementationId::from("conduit.proof/datagram@1");

    let mut invalid = binding();
    invalid.attachment.base = base.clone();
    invalid.attachment.contract.traffic_shape = LineTrafficShape::Datagram;
    assert_eq!(invalid.validate(), Err(WireError::InvalidBase));

    let mut connection = planned_connection(&invalid, base);
    connection
        .selected_line
        .as_mut()
        .unwrap()
        .contract
        .traffic_shape = LineTrafficShape::Datagram;
    connection.admitted_lines[0].contract.traffic_shape = LineTrafficShape::Datagram;
    assert_eq!(
        SessionBinding::from_planned_connection(
            invalid.plan_id,
            invalid.source_fragment_id,
            invalid.sink_fragment_id,
            &connection,
        ),
        Err(WireError::InvalidSession)
    );
}

#[test]
fn unsealed_selected_line_remains_rejected() {
    let expected = binding();
    let mut connection = planned_connection(
        &expected,
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
    );
    connection.selected_line.as_mut().unwrap().line_id = LineId::from("test/unsealed-line");
    assert_eq!(
        SessionBinding::from_planned_connection(
            expected.plan_id,
            expected.source_fragment_id,
            expected.sink_fragment_id,
            &connection,
        ),
        Err(WireError::InvalidSession)
    );
}

#[test]
fn mutated_base_in_peer_hello_remains_rejected() {
    let binding = binding();
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
    let mut hello = match binding.hello_frame().message {
        SessionMessage::Hello(hello) => hello,
        _ => unreachable!(),
    };
    hello.base = "conduit.proof/frame@1"; // Peer Hello specifies different base
    assert_eq!(
        machine.admit_inbound(binding.frame(SessionMessage::Hello(hello))),
        Err(WireError::InvalidSession)
    );
}

#[test]
fn mutated_base_instance_in_route_attachment_remains_rejected() {
    let binding = binding();
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
    let mut frame = binding.hello_frame();
    if let SessionMessage::Hello(ref mut hello) = frame.message {
        hello.base_instance_id = "test/different-base-instance";
    }
    assert_eq!(
        machine.admit_inbound(frame),
        Err(WireError::SessionEpochMismatch)
    );
}

#[test]
fn arbitrary_open_world_base_identity_round_trips_through_session_wire() {
    let mut b = binding();
    b.attachment.base = BaseImplementationId::from("third.party.base/quic@1");

    let hello_frame = b.hello_frame();
    let mut buf = [0u8; 1024];
    let encoded_len = encode_session_frame_into(
        hello_frame,
        &mut buf,
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .expect("encode hello frame");
    let decoded_frame = decode_session_frame(
        &buf[..encoded_len],
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .expect("decode hello frame");

    let hello_msg = match decoded_frame.message {
        SessionMessage::Hello(hello) => hello,
        _ => panic!("expected Hello message"),
    };
    assert_eq!(hello_msg.base, "third.party.base/quic@1");
    assert_eq!(hello_msg.contract, remote_session_contract());

    let mut machine = SessionMachine::new(b.clone(), SessionRole::Source).unwrap();
    machine.admit_outbound(hello_frame).unwrap();
    machine.admit_inbound(hello_frame).unwrap();
    machine
        .admit_outbound(b.frame(SessionMessage::Ready))
        .unwrap();
    machine
        .admit_inbound(b.frame(SessionMessage::Ready))
        .unwrap();
    assert!(machine.is_active());
}

#[test]
fn webrtc_datachannel_base_round_trips_through_session_wire() {
    let mut binding = binding();
    binding.attachment.base = BaseImplementationId::from("conduit.base/webrtc-data-channel@1");

    let mut bytes = [0; 1024];
    let length = encode_session_frame_into(
        binding.hello_frame(),
        &mut bytes,
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .expect("encode WebRTC Hello");
    let decoded =
        decode_session_frame(&bytes[..length], MAXIMUM_PAYLOAD_BYTES, MAXIMUM_FRAME_BYTES)
            .expect("decode WebRTC Hello");
    let SessionMessage::Hello(hello) = decoded.message else {
        panic!("expected Hello");
    };
    assert_eq!(hello.base, "conduit.base/webrtc-data-channel@1");
    assert_eq!(hello.contract, remote_session_contract());
}
