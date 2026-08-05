//! Adversarial wire-corpus integration tests.
//!
//! Every test operates on the binary fixtures under `fixtures/wire-corpus/` so
//! that the same vectors can be consumed by native Rust code *and* by WASM
//! consumers without re-deriving them.  The fixture layout is:
//!
//! ```text
//! fixtures/wire-corpus/
//!   golden.bin               — canonical deterministic round-trip vector
//!   empty_payload.bin        — zero-length payload
//!   max_payload.bin          — maximum legal payload (64 bytes)
//!   zero_capacity_ids.bin    — plan / conn / kind all empty strings
//!   max_id.bin               — all three identifiers at MAX_ID_BYTES (4 096)
//!   trunc_NNN.bin            — golden frame truncated after byte NNN
//!   oversized_frame.bin      — byte count exceeds FIXED + 3×MAX_ID + MAX_PAYLOAD
//!   oversized_payload.bin    — encoded payload_len field > MAX_PAYLOAD
//!   trailing_bytes.bin       — golden frame + two extra bytes
//!   wrong_magic.bin          — first byte changed from 'C' to 'X'
//!   wrong_wire_version.bin   — wire-format byte changed to 0xFF
//!   wrong_protocol_version.bin — protocol_version field = PROTOCOL_VERSION + 1
//!   wrong_sequence.bin       — different sequence; still valid (no seq validation)
//!   non_utf8_plan.bin        — plan_id bytes are not valid UTF-8
//!   id_length_overflow.bin   — plan_id declared length of 4 097 (> MAX_ID_BYTES)
//! ```

use conduit_wire::{decode_envelope, encode_envelope, WireError, MAX_ID_BYTES};

use conduit_core::{ConnectionEnvelope, ConnectionId, KindId, PlanId, PROTOCOL_VERSION};

/// Path to the shared fixture directory, relative to the workspace root.
fn corpus_dir() -> std::path::PathBuf {
    // Integration tests run with cwd = workspace root; fall back gracefully.
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // conduit-wire → crates
    p.pop(); // crates       → workspace root
    p.join("fixtures/wire-corpus")
}

fn fixture(name: &str) -> Vec<u8> {
    let path = corpus_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read fixture {name}: {e}"))
}

/// The `MAX_PAYLOAD` used when generating the corpus.
const MAX_PAYLOAD: u32 = 64;

/// Reference envelope that produced `golden.bin`.
fn golden_envelope() -> ConnectionEnvelope {
    ConnectionEnvelope {
        protocol_version: PROTOCOL_VERSION,
        plan_id: PlanId::from("plan-a"),
        connection_id: ConnectionId::from("conn-b"),
        sequence: 0x0102_0304_0506_0708,
        value_kind: KindId::from("test/value"),
        payload: vec![0x00, 0x01, 0x02, 0xff],
    }
}

// ---------------------------------------------------------------------------
// Deterministic round-trip vectors
// ---------------------------------------------------------------------------

#[test]
fn golden_fixture_round_trips() {
    let bytes = fixture("golden.bin");
    let env = decode_envelope(&bytes, MAX_PAYLOAD).expect("golden.bin must decode");
    assert_eq!(env, golden_envelope(), "decoded fields must match reference");

    let re_encoded = encode_envelope(&env, MAX_PAYLOAD).expect("round-trip encode");
    assert_eq!(re_encoded, bytes, "re-encoded bytes must be bit-for-bit identical");
}

#[test]
fn golden_fixture_byte_vector_is_stable() {
    // Explicit byte-level check so any wire-format change is immediately
    // visible as a test failure rather than a silent round-trip.
    let bytes = fixture("golden.bin");
    assert_eq!(
        bytes.as_slice(),
        b"CNDW\x01\x01\x00\x06\x00plan-a\x06\x00conn-b\x08\x07\x06\x05\x04\x03\x02\x01\x0a\x00test/value\x04\x00\x00\x00\x00\x01\x02\xff"
    );
}

#[test]
fn empty_payload_fixture_round_trips() {
    let bytes = fixture("empty_payload.bin");
    let env = decode_envelope(&bytes, MAX_PAYLOAD).expect("empty_payload.bin must decode");
    assert!(env.payload.is_empty());
    assert_eq!(encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"), bytes);
}

#[test]
fn max_payload_fixture_round_trips() {
    let bytes = fixture("max_payload.bin");
    let env = decode_envelope(&bytes, MAX_PAYLOAD).expect("max_payload.bin must decode");
    assert_eq!(env.payload.len(), MAX_PAYLOAD as usize);
    assert_eq!(encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"), bytes);
}

#[test]
fn zero_capacity_ids_fixture_round_trips() {
    let bytes = fixture("zero_capacity_ids.bin");
    let env = decode_envelope(&bytes, MAX_PAYLOAD).expect("zero_capacity_ids.bin must decode");
    assert_eq!(env.plan_id.as_str(), "");
    assert_eq!(env.connection_id.as_str(), "");
    assert_eq!(env.value_kind.as_str(), "");
    assert_eq!(encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"), bytes);
}

#[test]
fn max_id_fixture_round_trips() {
    let bytes = fixture("max_id.bin");
    let env =
        decode_envelope(&bytes, MAX_PAYLOAD).expect("max_id.bin must decode");
    assert_eq!(env.plan_id.as_str().len(), MAX_ID_BYTES);
    assert_eq!(env.connection_id.as_str().len(), MAX_ID_BYTES);
    assert_eq!(env.value_kind.as_str().len(), MAX_ID_BYTES);
    // Re-encode needs a generous frame budget
    let re = encode_envelope(&env, MAX_PAYLOAD).expect("re-encode");
    assert_eq!(
        decode_envelope(&re, MAX_PAYLOAD).expect("second decode"),
        env
    );
}

// ---------------------------------------------------------------------------
// Truncation at every field boundary
// ---------------------------------------------------------------------------

#[test]
fn every_truncation_is_rejected() {
    // golden.bin is 51 bytes; trunc_000 through trunc_050 cover all prefixes
    // shorter than the complete frame — none of them must decode successfully.
    let golden = fixture("golden.bin");
    for n in 0..golden.len() {
        let name = format!("trunc_{n:03}.bin");
        let bytes = fixture(&name);
        assert_eq!(bytes.len(), n, "fixture {name} should be {n} bytes");
        let result = decode_envelope(&bytes, MAX_PAYLOAD);
        assert!(
            result.is_err(),
            "trunc_{n:03}.bin (len={n}) decoded successfully but must fail"
        );
    }
}

// ---------------------------------------------------------------------------
// Oversized frame and payload
// ---------------------------------------------------------------------------

#[test]
fn oversized_frame_fixture_is_rejected() {
    let bytes = fixture("oversized_frame.bin");
    assert_eq!(
        decode_envelope(&bytes, MAX_PAYLOAD),
        Err(WireError::OversizedFrame)
    );
}

#[test]
fn oversized_payload_fixture_is_rejected() {
    let bytes = fixture("oversized_payload.bin");
    // The fixture was encoded with MAX_PAYLOAD+1 bytes; decoding with MAX_PAYLOAD must fail.
    assert_eq!(
        decode_envelope(&bytes, MAX_PAYLOAD),
        Err(WireError::OversizedPayload)
    );
}

#[test]
fn encode_rejects_oversized_payload() {
    let mut env = golden_envelope();
    env.payload = vec![0u8; MAX_PAYLOAD as usize + 1];
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD),
        Err(WireError::OversizedPayload)
    );
}

// ---------------------------------------------------------------------------
// Trailing bytes
// ---------------------------------------------------------------------------

#[test]
fn trailing_bytes_fixture_is_rejected() {
    let bytes = fixture("trailing_bytes.bin");
    assert_eq!(
        decode_envelope(&bytes, MAX_PAYLOAD),
        Err(WireError::TrailingGarbage)
    );
}

#[test]
fn single_trailing_byte_is_rejected() {
    let mut golden = fixture("golden.bin");
    golden.push(0x00);
    assert_eq!(
        decode_envelope(&golden, MAX_PAYLOAD),
        Err(WireError::TrailingGarbage)
    );
}

// ---------------------------------------------------------------------------
// Wrong magic / wire version / protocol version
// ---------------------------------------------------------------------------

#[test]
fn wrong_magic_fixture_is_rejected() {
    let bytes = fixture("wrong_magic.bin");
    assert_eq!(
        decode_envelope(&bytes, MAX_PAYLOAD),
        Err(WireError::InvalidMagic)
    );
}

#[test]
fn wrong_wire_version_fixture_is_rejected() {
    let bytes = fixture("wrong_wire_version.bin");
    assert_eq!(
        decode_envelope(&bytes, MAX_PAYLOAD),
        Err(WireError::UnsupportedWireFormat)
    );
}

#[test]
fn wrong_protocol_version_fixture_is_rejected() {
    let bytes = fixture("wrong_protocol_version.bin");
    assert_eq!(
        decode_envelope(&bytes, MAX_PAYLOAD),
        Err(WireError::WrongProtocolVersion)
    );
}

#[test]
fn encode_rejects_wrong_protocol_version() {
    let mut env = golden_envelope();
    env.protocol_version = PROTOCOL_VERSION + 1;
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD),
        Err(WireError::WrongProtocolVersion)
    );
}

// ---------------------------------------------------------------------------
// Wrong plan / connection / kind (invalid encoding)
// ---------------------------------------------------------------------------

#[test]
fn non_utf8_plan_id_is_rejected() {
    let bytes = fixture("non_utf8_plan.bin");
    assert_eq!(
        decode_envelope(&bytes, MAX_PAYLOAD),
        Err(WireError::InvalidIdentifierEncoding)
    );
}

#[test]
fn id_length_overflow_is_rejected() {
    let bytes = fixture("id_length_overflow.bin");
    // The declared plan_id length is 4097 > MAX_ID_BYTES, so we get IdentifierTooLong.
    // The actual bytes aren't present so TruncatedFrame is also acceptable.
    let err = decode_envelope(&bytes, MAX_PAYLOAD).expect_err("must fail");
    assert!(
        matches!(err, WireError::IdentifierTooLong | WireError::TruncatedFrame),
        "expected IdentifierTooLong or TruncatedFrame, got {err:?}"
    );
}

#[test]
fn encode_rejects_oversized_plan_id() {
    let mut env = golden_envelope();
    env.plan_id = PlanId::from("x".repeat(MAX_ID_BYTES + 1).as_str());
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD),
        Err(WireError::IdentifierTooLong)
    );
}

#[test]
fn encode_rejects_oversized_connection_id() {
    let mut env = golden_envelope();
    env.connection_id = ConnectionId::from("y".repeat(MAX_ID_BYTES + 1).as_str());
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD),
        Err(WireError::IdentifierTooLong)
    );
}

#[test]
fn encode_rejects_oversized_kind_id() {
    let mut env = golden_envelope();
    env.value_kind = KindId::from("z".repeat(MAX_ID_BYTES + 1).as_str());
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD),
        Err(WireError::IdentifierTooLong)
    );
}

// ---------------------------------------------------------------------------
// Wrong / unusual sequence numbers
// ---------------------------------------------------------------------------

#[test]
fn wrong_sequence_fixture_still_decodes() {
    // The wire format carries sequence as an opaque u64; there is no validity
    // check, so any value must be accepted.
    let bytes = fixture("wrong_sequence.bin");
    let env = decode_envelope(&bytes, MAX_PAYLOAD).expect("arbitrary sequence must decode");
    assert_eq!(env.sequence, 0xDEAD_BEEF_CAFE_BABE_u64);
}

#[test]
fn sequence_zero_round_trips() {
    let mut env = golden_envelope();
    env.sequence = 0;
    let encoded = encode_envelope(&env, MAX_PAYLOAD).expect("seq=0 encodes");
    let decoded = decode_envelope(&encoded, MAX_PAYLOAD).expect("seq=0 decodes");
    assert_eq!(decoded.sequence, 0);
}

#[test]
fn sequence_max_round_trips() {
    let mut env = golden_envelope();
    env.sequence = u64::MAX;
    let encoded = encode_envelope(&env, MAX_PAYLOAD).expect("seq=MAX encodes");
    let decoded = decode_envelope(&encoded, MAX_PAYLOAD).expect("seq=MAX decodes");
    assert_eq!(decoded.sequence, u64::MAX);
}

// ---------------------------------------------------------------------------
// Duplicate and reordered frames
// ---------------------------------------------------------------------------

#[test]
fn duplicate_frames_each_decode_independently() {
    // The codec is stateless; duplicate frames must each decode to the same value.
    let bytes = fixture("golden.bin");
    let first = decode_envelope(&bytes, MAX_PAYLOAD).expect("first decode");
    let second = decode_envelope(&bytes, MAX_PAYLOAD).expect("second decode");
    assert_eq!(first, second, "duplicate frames must produce equal results");
}

#[test]
fn reordered_frames_each_decode_independently() {
    // Encode two distinct envelopes, then decode them in reverse order.
    let mut env_a = golden_envelope();
    env_a.sequence = 1;
    let mut env_b = golden_envelope();
    env_b.sequence = 2;
    env_b.payload = vec![0xAA, 0xBB];

    let bytes_a = encode_envelope(&env_a, MAX_PAYLOAD).expect("env_a encodes");
    let bytes_b = encode_envelope(&env_b, MAX_PAYLOAD).expect("env_b encodes");

    // Decode in reverse (B then A) — both must succeed with the right content.
    let decoded_b = decode_envelope(&bytes_b, MAX_PAYLOAD).expect("env_b decodes");
    let decoded_a = decode_envelope(&bytes_a, MAX_PAYLOAD).expect("env_a decodes");

    assert_eq!(decoded_a.sequence, 1);
    assert_eq!(decoded_b.sequence, 2);
    assert_eq!(decoded_b.payload, vec![0xAA, 0xBB]);
}

// ---------------------------------------------------------------------------
// Zero and maximum legal capacities
// ---------------------------------------------------------------------------

#[test]
fn zero_maximum_payload_bytes_accepts_empty_payload() {
    let mut env = golden_envelope();
    env.payload = vec![];
    let encoded = encode_envelope(&env, 0).expect("zero capacity with empty payload encodes");
    let decoded = decode_envelope(&encoded, 0).expect("zero capacity with empty payload decodes");
    assert!(decoded.payload.is_empty());
}

#[test]
fn zero_maximum_payload_bytes_rejects_nonempty_payload() {
    let env = golden_envelope(); // payload is non-empty
    assert_eq!(
        encode_envelope(&env, 0),
        Err(WireError::OversizedPayload)
    );
}

#[test]
fn maximum_legal_payload_capacity_round_trips() {
    let mut env = golden_envelope();
    env.payload = vec![0xFFu8; u16::MAX as usize];
    let cap = u16::MAX as u32;
    let encoded = encode_envelope(&env, cap).expect("max u16 payload encodes");
    let decoded = decode_envelope(&encoded, cap).expect("max u16 payload decodes");
    assert_eq!(decoded.payload.len(), u16::MAX as usize);
}
