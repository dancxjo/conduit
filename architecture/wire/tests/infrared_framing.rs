use conduit_core::{
    ConnectionEnvelope, ConnectionId, KindId, LineDuplex, LineReliability, LineTrafficShape,
    PlanId, PROTOCOL_VERSION,
};
use conduit_wire::{
    decode_infrared_frame, encode_envelope, encode_infrared_frame, encode_session_frame_into,
    InfraredFrameError, InfraredProfileError, InfraredReceiveSequence, InfraredSimplexProfile,
    SessionFrame, SessionIdentity, SessionLimits, SessionMessage, INFRARED_MAXIMUM_FRAME_BYTES,
    INFRARED_MAXIMUM_PAYLOAD_BYTES,
};

#[test]
fn first_profile_is_exact_simplex_best_effort_and_finite() {
    let profile = InfraredSimplexProfile::FIRST.validate().unwrap();
    let contract = InfraredSimplexProfile::line_contract();
    let limits = profile.link_limits().unwrap();

    assert_eq!(contract.traffic_shape, LineTrafficShape::Message);
    assert_eq!(contract.duplex, LineDuplex::Simplex);
    assert_eq!(contract.reliability, LineReliability::BestEffort);
    assert_eq!(limits.maximum_in_flight_items, 1);
    assert_eq!(
        limits.maximum_payload_bytes,
        INFRARED_MAXIMUM_PAYLOAD_BYTES as u32
    );
    assert_eq!(
        limits.maximum_frame_bytes,
        INFRARED_MAXIMUM_FRAME_BYTES as u32
    );
    assert_eq!(
        limits.maximum_buffered_bytes,
        limits.maximum_frame_bytes * 2
    );
    assert_eq!(profile.transmit_queue_items, 1);
    assert_eq!(profile.receive_queue_items, 1);
    assert_eq!(profile.requirements.carrier_hz, 38_000);
    assert_eq!(profile.requirements.inter_frame_gap_micros, 20_000);
}

#[test]
fn ordinary_data_and_session_envelopes_fit_and_round_trip() {
    let data = ConnectionEnvelope {
        protocol_version: PROTOCOL_VERSION,
        plan_id: PlanId::from("plan/infrared"),
        connection_id: ConnectionId::from("cord/infrared"),
        sequence: 7,
        value_kind: KindId::from("text/utf8"),
        payload: b"bounded optical value".to_vec(),
    };
    let encoded_data = encode_envelope(&data, 96).unwrap();

    let identity = SessionIdentity {
        protocol_version: PROTOCOL_VERSION,
        plan_id: "plan/infrared",
        source_fragment_id: "fragment/source",
        sink_fragment_id: "fragment/sink",
        source_active_play_id: "play/source",
        sink_active_play_id: "play/sink",
        connection_id: "cord/infrared",
        source_host_id: "host/source",
        source_boot_id: "boot/source",
        sink_host_id: "host/sink",
        sink_boot_id: "boot/sink",
        value_kind: "text/utf8",
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 96,
            maximum_buffered_bytes: 96,
        },
    };
    let mut encoded_session = [0_u8; INFRARED_MAXIMUM_PAYLOAD_BYTES];
    let session_length = encode_session_frame_into(
        SessionFrame {
            identity,
            message: SessionMessage::Offered {
                sequence: 0,
                payload: b"bounded optical value",
            },
        },
        &mut encoded_session,
        96,
        INFRARED_MAXIMUM_PAYLOAD_BYTES as u32,
    )
    .unwrap();

    for payload in [encoded_data.as_slice(), &encoded_session[..session_length]] {
        let mut optical = [0_u8; INFRARED_MAXIMUM_FRAME_BYTES];
        let length =
            encode_infrared_frame(payload, 0, InfraredSimplexProfile::FIRST, &mut optical).unwrap();
        let decoded =
            decode_infrared_frame(&optical[..length], InfraredSimplexProfile::FIRST).unwrap();
        assert_eq!(decoded.sequence, 0);
        assert_eq!(decoded.payload, payload);
    }
}

#[test]
fn malformed_incomplete_unsupported_and_ordering_states_refuse_distinctly() {
    let profile = InfraredSimplexProfile::FIRST;
    let mut bytes = [0_u8; INFRARED_MAXIMUM_FRAME_BYTES];
    let length = encode_infrared_frame(b"one", 0, profile, &mut bytes).unwrap();

    assert_eq!(
        decode_infrared_frame(&bytes[..length - 1], profile),
        Err(InfraredFrameError::IncompleteFrame)
    );

    let mut invalid_preamble = bytes;
    invalid_preamble[0] ^= 1;
    assert_eq!(
        decode_infrared_frame(&invalid_preamble[..length], profile),
        Err(InfraredFrameError::InvalidPreamble)
    );

    let mut invalid_integrity = bytes;
    invalid_integrity[10] ^= 1;
    assert_eq!(
        decode_infrared_frame(&invalid_integrity[..length], profile),
        Err(InfraredFrameError::IntegrityMismatch)
    );

    let mut wrong_revision = profile;
    wrong_revision.revision = 2;
    assert_eq!(
        wrong_revision.validate(),
        Err(InfraredProfileError::UnsupportedRevision)
    );
    let mut wrong_profile = profile;
    wrong_profile.profile = 2;
    assert_eq!(
        wrong_profile.validate(),
        Err(InfraredProfileError::UnsupportedProfile)
    );

    assert_eq!(
        InfraredReceiveSequence::admit_gap(4),
        Err(InfraredFrameError::IncompleteFrame)
    );
    assert_eq!(InfraredReceiveSequence::admit_gap(0), Ok(()));

    let frame = decode_infrared_frame(&bytes[..length], profile).unwrap();
    let mut receiver = InfraredReceiveSequence::new();
    assert_eq!(receiver.admit(frame), Ok(b"one".as_slice()));
    assert_eq!(
        receiver.admit(frame),
        Err(InfraredFrameError::DuplicateFrame)
    );

    let next_length = encode_infrared_frame(b"three", 2, profile, &mut bytes).unwrap();
    let future = decode_infrared_frame(&bytes[..next_length], profile).unwrap();
    assert_eq!(
        receiver.admit(future),
        Err(InfraredFrameError::ReorderedFrame)
    );
}

#[test]
fn capacity_and_profile_mutations_fail_closed() {
    let profile = InfraredSimplexProfile::FIRST;
    assert_eq!(
        encode_infrared_frame(&[], 0, profile, &mut [0_u8; 16]),
        Err(InfraredFrameError::EmptyPayload)
    );
    assert_eq!(
        encode_infrared_frame(
            &[1_u8; INFRARED_MAXIMUM_PAYLOAD_BYTES + 1],
            0,
            profile,
            &mut [0_u8; INFRARED_MAXIMUM_FRAME_BYTES]
        ),
        Err(InfraredFrameError::OversizedPayload)
    );
    assert_eq!(
        encode_infrared_frame(b"value", 0, profile, &mut [0_u8; 4]),
        Err(InfraredFrameError::OutputTooSmall)
    );

    let mut extra_queue = profile;
    extra_queue.receive_queue_items = 2;
    assert_eq!(
        extra_queue.validate(),
        Err(InfraredProfileError::InvalidQueueLimit)
    );
    let mut invalid_carrier = profile;
    invalid_carrier.requirements.carrier_hz = 0;
    assert_eq!(
        invalid_carrier.validate(),
        Err(InfraredProfileError::InvalidCarrier)
    );
}
