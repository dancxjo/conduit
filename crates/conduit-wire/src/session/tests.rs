use super::*;
use conduit_core::{
    BootId, HostId, LinkAuthorityReference, LinkAvailability, LinkBinding, LinkCredentialReference,
    LinkEndpointId, PlacementId,
};

const MAXIMUM_PAYLOAD_BYTES: u32 = 16;
const MAXIMUM_FRAME_BYTES: u32 = 512;

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
        link_binding_id: LinkBindingId::from("test/link"),
        provider: ConnectionProvider::WebSocket,
        provider_instance_id: ConnectionProviderInstanceId::from("test/provider-instance"),
        source,
        sink,
        value_kind: KindId::from("test/value"),
        limits: LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: MAXIMUM_PAYLOAD_BYTES,
            maximum_buffered_bytes: MAXIMUM_PAYLOAD_BYTES,
            maximum_frame_bytes: MAXIMUM_FRAME_BYTES,
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
    assert!(machine.is_active());
}

#[test]
fn exact_planned_connection_constructs_the_session_binding() {
    let expected = binding();
    let connection = PlannedConnection {
        connection_id: expected.connection_id.clone(),
        source_placement_id: PlacementId::from("test/source-placement"),
        source_port_id: conduit_core::PortId::from("out"),
        sink_placement_id: PlacementId::from("test/sink-placement"),
        sink_port_id: conduit_core::PortId::from("in"),
        value_kind: expected.value_kind.clone(),
        provider: ConnectionProvider::WebSocket,
        link_binding: Some(LinkBinding {
            binding_id: expected.link_binding_id.clone(),
            source: expected.source.clone(),
            sink: expected.sink.clone(),
            provider: expected.provider,
            provider_instance_id: expected.provider_instance_id.clone(),
            availability: LinkAvailability::Ready,
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: expected.limits,
        }),
        item_capacity: 1,
        byte_capacity: MAXIMUM_PAYLOAD_BYTES,
    };
    let actual = SessionBinding::from_planned_connection(
        expected.plan_id.clone(),
        expected.source_fragment_id.clone(),
        expected.sink_fragment_id.clone(),
        &connection,
    )
    .unwrap();
    assert_eq!(actual, expected);

    let mut missing = connection.clone();
    missing.link_binding = None;
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
    activate(&mut machine);

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
    activate(&mut source);
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
    activate(&mut machine);
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
        Err(WireError::InvalidSession)
    );

    let mut wrong_hello = match binding.hello_frame().message {
        SessionMessage::Hello(hello) => hello,
        _ => unreachable!(),
    };
    wrong_hello.limits.maximum_payload_bytes += 1;
    assert_eq!(
        machine.admit_inbound(binding.frame(SessionMessage::Hello(wrong_hello))),
        Err(WireError::InvalidSession)
    );

    let mut invalid = binding;
    invalid.limits.maximum_frame_bytes = 32;
    assert!(matches!(
        SessionMachine::new(invalid, SessionRole::Source),
        Err(WireError::InvalidLimits)
    ));
}

#[test]
fn every_routing_and_value_identity_mutation_fails_closed() {
    let binding = binding();
    for field in 0..8 {
        let mut frame = binding.hello_frame();
        match field {
            0 => frame.identity.plan_id = "wrong/plan",
            1 => frame.identity.source_fragment_id = "wrong/source-fragment",
            2 => frame.identity.sink_fragment_id = "wrong/sink-fragment",
            3 => frame.identity.connection_id = "wrong/connection",
            4 => frame.identity.source_active_play_id = "wrong/source-play",
            5 => frame.identity.sink_active_play_id = "wrong/sink-play",
            6 => frame.identity.link_binding_id = "wrong/link",
            7 => frame.identity.provider_instance_id = "wrong/provider-instance",
            _ => unreachable!(),
        }
        let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
        assert_eq!(machine.admit_inbound(frame), Err(WireError::InvalidSession));
    }
    let mut hello = match binding.hello_frame().message {
        SessionMessage::Hello(hello) => hello,
        _ => unreachable!(),
    };
    hello.value_kind = "wrong/value-kind";
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
    assert_eq!(
        machine.admit_inbound(binding.frame(SessionMessage::Hello(hello))),
        Err(WireError::InvalidSession)
    );
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
    // protocol. This proves session carrier neutrality without claiming that FixtureFrame
    // is an installed, runnable, or production-ready physical transport.
    let mut expected = binding();
    expected.provider = ConnectionProvider::FixtureFrame;
    assert!(expected.provider.supports_remote_session());

    let connection = PlannedConnection {
        connection_id: expected.connection_id.clone(),
        source_placement_id: PlacementId::from("test/source-placement"),
        source_port_id: conduit_core::PortId::from("out"),
        sink_placement_id: PlacementId::from("test/sink-placement"),
        sink_port_id: conduit_core::PortId::from("in"),
        value_kind: expected.value_kind.clone(),
        provider: ConnectionProvider::FixtureFrame,
        link_binding: Some(LinkBinding {
            binding_id: expected.link_binding_id.clone(),
            source: expected.source.clone(),
            sink: expected.sink.clone(),
            provider: ConnectionProvider::FixtureFrame,
            provider_instance_id: expected.provider_instance_id.clone(),
            availability: LinkAvailability::Ready,
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: expected.limits,
        }),
        item_capacity: 1,
        byte_capacity: MAXIMUM_PAYLOAD_BYTES,
    };

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
    activate(&mut machine);
    assert!(machine.is_active());
}

#[test]
fn local_and_in_memory_providers_remain_rejected() {
    let base = binding();
    for provider in [ConnectionProvider::Local, ConnectionProvider::InMemory] {
        assert!(!provider.supports_remote_session());
        let mut invalid = base.clone();
        invalid.provider = provider;
        assert_eq!(invalid.validate(), Err(WireError::InvalidProvider));

        let connection = PlannedConnection {
            connection_id: base.connection_id.clone(),
            source_placement_id: PlacementId::from("test/source-placement"),
            source_port_id: conduit_core::PortId::from("out"),
            sink_placement_id: PlacementId::from("test/sink-placement"),
            sink_port_id: conduit_core::PortId::from("in"),
            value_kind: base.value_kind.clone(),
            provider,
            link_binding: Some(LinkBinding {
                binding_id: base.link_binding_id.clone(),
                source: base.source.clone(),
                sink: base.sink.clone(),
                provider,
                provider_instance_id: base.provider_instance_id.clone(),
                availability: LinkAvailability::Ready,
                credential: LinkCredentialReference::None,
                authority: LinkAuthorityReference::ProcessOwned,
                limits: base.limits,
            }),
            item_capacity: 1,
            byte_capacity: MAXIMUM_PAYLOAD_BYTES,
        };
        assert_eq!(
            SessionBinding::from_planned_connection(
                base.plan_id.clone(),
                base.source_fragment_id.clone(),
                base.sink_fragment_id.clone(),
                &connection,
            ),
            Err(WireError::InvalidSession)
        );
    }
}

#[test]
fn fixture_datagram_provider_remains_rejected() {
    // Explicit negative proving datagram fixtures are not promoted into frame wire sessions
    // and that eligibility is an explicit allowlist rather than merely provider != Local.
    let provider = ConnectionProvider::FixtureDatagram;
    assert!(!provider.supports_remote_session());

    let mut invalid = binding();
    invalid.provider = provider;
    assert_eq!(invalid.validate(), Err(WireError::InvalidProvider));

    let connection = PlannedConnection {
        connection_id: invalid.connection_id.clone(),
        source_placement_id: PlacementId::from("test/source-placement"),
        source_port_id: conduit_core::PortId::from("out"),
        sink_placement_id: PlacementId::from("test/sink-placement"),
        sink_port_id: conduit_core::PortId::from("in"),
        value_kind: invalid.value_kind.clone(),
        provider,
        link_binding: Some(LinkBinding {
            binding_id: invalid.link_binding_id.clone(),
            source: invalid.source.clone(),
            sink: invalid.sink.clone(),
            provider,
            provider_instance_id: invalid.provider_instance_id.clone(),
            availability: LinkAvailability::Ready,
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: invalid.limits,
        }),
        item_capacity: 1,
        byte_capacity: MAXIMUM_PAYLOAD_BYTES,
    };
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
fn provider_link_mismatch_remains_rejected() {
    let expected = binding();
    let connection = PlannedConnection {
        connection_id: expected.connection_id.clone(),
        source_placement_id: PlacementId::from("test/source-placement"),
        source_port_id: conduit_core::PortId::from("out"),
        sink_placement_id: PlacementId::from("test/sink-placement"),
        sink_port_id: conduit_core::PortId::from("in"),
        value_kind: expected.value_kind.clone(),
        provider: ConnectionProvider::WebSocket,
        link_binding: Some(LinkBinding {
            binding_id: expected.link_binding_id.clone(),
            source: expected.source.clone(),
            sink: expected.sink.clone(),
            provider: ConnectionProvider::FixtureFrame, // Mismatched link provider
            provider_instance_id: expected.provider_instance_id.clone(),
            availability: LinkAvailability::Ready,
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: expected.limits,
        }),
        item_capacity: 1,
        byte_capacity: MAXIMUM_PAYLOAD_BYTES,
    };
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
fn mutated_provider_in_peer_hello_remains_rejected() {
    let binding = binding();
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
    let mut hello = match binding.hello_frame().message {
        SessionMessage::Hello(hello) => hello,
        _ => unreachable!(),
    };
    hello.provider = ConnectionProvider::FixtureFrame; // Peer Hello specifies different provider
    assert_eq!(
        machine.admit_inbound(binding.frame(SessionMessage::Hello(hello))),
        Err(WireError::InvalidSession)
    );
}

#[test]
fn mutated_provider_instance_identity_in_session_identity_remains_rejected() {
    let binding = binding();
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
    let mut frame = binding.hello_frame();
    frame.identity.provider_instance_id = "test/different-provider-instance";
    assert_eq!(machine.admit_inbound(frame), Err(WireError::InvalidSession));
}
