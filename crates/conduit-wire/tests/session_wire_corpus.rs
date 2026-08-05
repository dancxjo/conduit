//! The live session protocol consumes the adversarial corpus introduced in
//! PR #405 instead of maintaining a second, hand-written mutation vocabulary.

use conduit_core::{
    bind_active_play, BootId, ConnectionEnvelope, ConnectionProvider, ConnectionProviderInstanceId,
    FragmentId, HostId, LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits, PROTOCOL_VERSION,
};
use conduit_wire::{
    decode_envelope, decode_session_frame, encode_session_frame_into, SessionBinding,
    SessionMachine, SessionMessage, SessionRole, WireError,
};

const CORPUS_MAXIMUM_PAYLOAD_BYTES: u32 = 64;
const SESSION_MAXIMUM_FRAME_BYTES: u32 = u16::MAX as u32;

macro_rules! corpus {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/wire-corpus/",
            $name
        ))
    };
}

const TRUNCATIONS: [&[u8]; 51] = [
    corpus!("trunc_000.bin"),
    corpus!("trunc_001.bin"),
    corpus!("trunc_002.bin"),
    corpus!("trunc_003.bin"),
    corpus!("trunc_004.bin"),
    corpus!("trunc_005.bin"),
    corpus!("trunc_006.bin"),
    corpus!("trunc_007.bin"),
    corpus!("trunc_008.bin"),
    corpus!("trunc_009.bin"),
    corpus!("trunc_010.bin"),
    corpus!("trunc_011.bin"),
    corpus!("trunc_012.bin"),
    corpus!("trunc_013.bin"),
    corpus!("trunc_014.bin"),
    corpus!("trunc_015.bin"),
    corpus!("trunc_016.bin"),
    corpus!("trunc_017.bin"),
    corpus!("trunc_018.bin"),
    corpus!("trunc_019.bin"),
    corpus!("trunc_020.bin"),
    corpus!("trunc_021.bin"),
    corpus!("trunc_022.bin"),
    corpus!("trunc_023.bin"),
    corpus!("trunc_024.bin"),
    corpus!("trunc_025.bin"),
    corpus!("trunc_026.bin"),
    corpus!("trunc_027.bin"),
    corpus!("trunc_028.bin"),
    corpus!("trunc_029.bin"),
    corpus!("trunc_030.bin"),
    corpus!("trunc_031.bin"),
    corpus!("trunc_032.bin"),
    corpus!("trunc_033.bin"),
    corpus!("trunc_034.bin"),
    corpus!("trunc_035.bin"),
    corpus!("trunc_036.bin"),
    corpus!("trunc_037.bin"),
    corpus!("trunc_038.bin"),
    corpus!("trunc_039.bin"),
    corpus!("trunc_040.bin"),
    corpus!("trunc_041.bin"),
    corpus!("trunc_042.bin"),
    corpus!("trunc_043.bin"),
    corpus!("trunc_044.bin"),
    corpus!("trunc_045.bin"),
    corpus!("trunc_046.bin"),
    corpus!("trunc_047.bin"),
    corpus!("trunc_048.bin"),
    corpus!("trunc_049.bin"),
    corpus!("trunc_050.bin"),
];

fn envelope(bytes: &[u8], name: &'static str) -> ConnectionEnvelope {
    decode_envelope(bytes, CORPUS_MAXIMUM_PAYLOAD_BYTES)
        .unwrap_or_else(|error| panic!("{name} must decode for the session vector: {error:?}"))
}

fn binding(envelope: &ConnectionEnvelope) -> SessionBinding {
    let source = LinkEndpoint {
        host_id: HostId::from("corpus/source-host"),
        boot_id: BootId::from("corpus/source-boot"),
        endpoint_id: LinkEndpointId::from("corpus/source-endpoint"),
    };
    let sink = LinkEndpoint {
        host_id: HostId::from("corpus/sink-host"),
        boot_id: BootId::from("corpus/sink-boot"),
        endpoint_id: LinkEndpointId::from("corpus/sink-endpoint"),
    };
    let source_active_play_id =
        bind_active_play(&envelope.plan_id, &source.host_id, &source.boot_id, 0).active_play_id;
    let sink_active_play_id =
        bind_active_play(&envelope.plan_id, &sink.host_id, &sink.boot_id, 0).active_play_id;
    SessionBinding {
        protocol_version: envelope.protocol_version,
        plan_id: envelope.plan_id.clone(),
        source_fragment_id: FragmentId::from("corpus/source-fragment"),
        sink_fragment_id: FragmentId::from("corpus/sink-fragment"),
        source_active_play_id,
        sink_active_play_id,
        connection_id: envelope.connection_id.clone(),
        link_binding_id: LinkBindingId::from("corpus/link"),
        provider: ConnectionProvider::WebSocket,
        provider_instance_id: ConnectionProviderInstanceId::from("corpus/provider"),
        source,
        sink,
        value_kind: envelope.value_kind.clone(),
        limits: LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: CORPUS_MAXIMUM_PAYLOAD_BYTES,
            maximum_buffered_bytes: CORPUS_MAXIMUM_PAYLOAD_BYTES,
            maximum_frame_bytes: SESSION_MAXIMUM_FRAME_BYTES,
        },
    }
}

fn activate(machine: &mut SessionMachine) {
    let exact = machine.binding().clone();
    machine.admit_outbound(exact.hello_frame()).unwrap();
    machine.admit_inbound(exact.hello_frame()).unwrap();
    machine
        .admit_outbound(exact.frame(SessionMessage::Ready))
        .unwrap();
    machine
        .admit_inbound(exact.frame(SessionMessage::Ready))
        .unwrap();
}

fn encode_offered(binding: &SessionBinding, sequence: u64, payload: &[u8]) -> Vec<u8> {
    let mut output = vec![0; SESSION_MAXIMUM_FRAME_BYTES as usize];
    let length = encode_session_frame_into(
        binding.frame(SessionMessage::Offered { sequence, payload }),
        &mut output,
        CORPUS_MAXIMUM_PAYLOAD_BYTES,
        SESSION_MAXIMUM_FRAME_BYTES,
    )
    .expect("session offer encodes");
    output.truncate(length);
    output
}

#[test]
fn valid_corpus_boundaries_drive_the_live_session_codec() {
    for (name, bytes) in [
        ("golden.bin", corpus!("golden.bin").as_slice()),
        ("empty_payload.bin", corpus!("empty_payload.bin").as_slice()),
        ("max_payload.bin", corpus!("max_payload.bin").as_slice()),
    ] {
        let envelope = envelope(bytes, name);
        let binding = binding(&envelope);
        let encoded = encode_offered(&binding, 0, &envelope.payload);
        let decoded = decode_session_frame(
            &encoded,
            CORPUS_MAXIMUM_PAYLOAD_BYTES,
            SESSION_MAXIMUM_FRAME_BYTES,
        )
        .expect("corpus-derived session offer decodes");
        assert_eq!(
            decoded,
            binding.frame(SessionMessage::Offered {
                sequence: 0,
                payload: &envelope.payload,
            }),
            "{name} must retain its exact payload at the live session boundary"
        );
    }

    let maximum = envelope(corpus!("max_id.bin"), "max_id.bin");
    let maximum_binding = binding(&maximum);
    let mut output = vec![0; SESSION_MAXIMUM_FRAME_BYTES as usize];
    let length = encode_session_frame_into(
        maximum_binding.hello_frame(),
        &mut output,
        CORPUS_MAXIMUM_PAYLOAD_BYTES,
        SESSION_MAXIMUM_FRAME_BYTES,
    )
    .expect("maximum corpus identifiers fit the explicitly admitted frame bound");
    assert_eq!(
        decode_session_frame(
            &output[..length],
            CORPUS_MAXIMUM_PAYLOAD_BYTES,
            SESSION_MAXIMUM_FRAME_BYTES,
        )
        .expect("maximum identifier hello decodes"),
        maximum_binding.hello_frame()
    );

    let zero = binding(&envelope(
        corpus!("zero_capacity_ids.bin"),
        "zero_capacity_ids.bin",
    ));
    assert!(matches!(
        SessionMachine::new(zero, SessionRole::Source),
        Err(WireError::InvalidSession)
    ));
}

#[test]
fn identity_and_sequence_mutation_corpus_drives_session_denial() {
    let baseline = binding(&envelope(corpus!("golden.bin"), "golden.bin"));

    for (name, bytes, field) in [
        ("wrong_plan.bin", corpus!("wrong_plan.bin").as_slice(), 0_u8),
        (
            "wrong_connection.bin",
            corpus!("wrong_connection.bin").as_slice(),
            1_u8,
        ),
    ] {
        let mutation = envelope(bytes, name);
        let mut frame = baseline.frame(SessionMessage::Offered {
            sequence: 0,
            payload: &mutation.payload,
        });
        if field == 0 {
            frame.identity.plan_id = mutation.plan_id.as_str();
        } else {
            frame.identity.connection_id = mutation.connection_id.as_str();
        }
        let mut machine = SessionMachine::new(baseline.clone(), SessionRole::Source).unwrap();
        activate(&mut machine);
        assert_eq!(
            machine.admit_outbound(frame),
            Err(WireError::InvalidSession),
            "{name} must fail at the stateful live session boundary"
        );
    }

    let wrong_kind = envelope(corpus!("wrong_kind.bin"), "wrong_kind.bin");
    let mut hello = match baseline.hello_frame().message {
        SessionMessage::Hello(hello) => hello,
        _ => unreachable!(),
    };
    hello.value_kind = wrong_kind.value_kind.as_str();
    let mut machine = SessionMachine::new(baseline.clone(), SessionRole::Source).unwrap();
    assert_eq!(
        machine.admit_inbound(baseline.frame(SessionMessage::Hello(hello))),
        Err(WireError::InvalidSession)
    );

    let wrong_sequence = envelope(corpus!("wrong_sequence.bin"), "wrong_sequence.bin");
    let mut machine = SessionMachine::new(baseline.clone(), SessionRole::Source).unwrap();
    activate(&mut machine);
    assert_eq!(
        machine.admit_outbound(baseline.frame(SessionMessage::Offered {
            sequence: wrong_sequence.sequence,
            payload: &wrong_sequence.payload,
        })),
        Err(WireError::ReorderedFrame)
    );

    let protocol = corpus!("wrong_protocol_version.bin");
    let wrong_protocol = u16::from_le_bytes([protocol[5], protocol[6]]);
    assert_ne!(wrong_protocol, PROTOCOL_VERSION);
    let mut frame = baseline.frame(SessionMessage::Ready);
    frame.identity.protocol_version = wrong_protocol;
    assert_eq!(
        encode_session_frame_into(
            frame,
            &mut [0; 256],
            CORPUS_MAXIMUM_PAYLOAD_BYTES,
            SESSION_MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::WrongProtocolVersion)
    );
}

#[test]
fn malformed_corpus_drives_live_session_codec_denial() {
    let golden = envelope(corpus!("golden.bin"), "golden.bin");
    let baseline = binding(&golden);
    let encoded = encode_offered(&baseline, 0, &golden.payload);

    let mut wrong_magic = encoded.clone();
    wrong_magic[..4].copy_from_slice(&corpus!("wrong_magic.bin")[..4]);
    assert_eq!(
        decode_session_frame(
            &wrong_magic,
            CORPUS_MAXIMUM_PAYLOAD_BYTES,
            SESSION_MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::InvalidMagic)
    );

    let mut wrong_wire = encoded.clone();
    wrong_wire[4] = corpus!("wrong_wire_version.bin")[4];
    assert_eq!(
        decode_session_frame(
            &wrong_wire,
            CORPUS_MAXIMUM_PAYLOAD_BYTES,
            SESSION_MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::UnsupportedWireFormat)
    );

    for fixture in TRUNCATIONS {
        let truncation = fixture.len();
        assert_eq!(
            decode_session_frame(
                &encoded[..truncation],
                CORPUS_MAXIMUM_PAYLOAD_BYTES,
                SESSION_MAXIMUM_FRAME_BYTES,
            ),
            Err(WireError::TruncatedFrame),
            "corpus truncation {truncation} must remain rejected"
        );
    }

    let oversized_frame = corpus!("oversized_frame.bin");
    assert_eq!(
        decode_session_frame(
            oversized_frame,
            CORPUS_MAXIMUM_PAYLOAD_BYTES,
            (oversized_frame.len() - 1) as u32,
        ),
        Err(WireError::OversizedFrame)
    );

    let oversized_payload = corpus!("oversized_payload.bin");
    let payload =
        &oversized_payload[oversized_payload.len() - (CORPUS_MAXIMUM_PAYLOAD_BYTES as usize + 1)..];
    assert_eq!(
        encode_session_frame_into(
            baseline.frame(SessionMessage::Offered {
                sequence: 0,
                payload,
            }),
            &mut vec![0; SESSION_MAXIMUM_FRAME_BYTES as usize],
            CORPUS_MAXIMUM_PAYLOAD_BYTES,
            SESSION_MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::OversizedPayload)
    );

    let trailing_fixture = corpus!("trailing_bytes.bin");
    let trailing_suffix = &trailing_fixture[corpus!("golden.bin").len()..];
    let mut trailing = encoded.clone();
    trailing.extend_from_slice(trailing_suffix);
    assert_eq!(
        decode_session_frame(
            &trailing,
            CORPUS_MAXIMUM_PAYLOAD_BYTES,
            SESSION_MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::TrailingGarbage)
    );

    let mut non_utf8 = encoded.clone();
    non_utf8[10] = corpus!("non_utf8_plan.bin")[9];
    assert_eq!(
        decode_session_frame(
            &non_utf8,
            CORPUS_MAXIMUM_PAYLOAD_BYTES,
            SESSION_MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::InvalidIdentifierEncoding)
    );

    let mut identifier_overflow = encoded;
    identifier_overflow[8..10].copy_from_slice(&corpus!("id_length_overflow.bin")[7..9]);
    assert_eq!(
        decode_session_frame(
            &identifier_overflow,
            CORPUS_MAXIMUM_PAYLOAD_BYTES,
            SESSION_MAXIMUM_FRAME_BYTES,
        ),
        Err(WireError::IdentifierTooLong)
    );
}
