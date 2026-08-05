//! WASM-compatible deterministic vector tests.
//!
//! This file uses only `include_bytes!` to embed the shared corpus fixtures so
//! the same vectors can be verified under `wasm32-unknown-unknown` without any
//! filesystem access.  Run the WASM compilation proof with:
//!
//! ```sh
//! cargo check -p conduit-wire --target wasm32-unknown-unknown --test wire_corpus_vectors
//! ```
//!
//! All assertions here mirror `wire_corpus.rs`; if a fixture changes both
//! files will fail in unison.

use conduit_core::{ConnectionEnvelope, ConnectionId, KindId, PlanId, PROTOCOL_VERSION};
use conduit_wire::{decode_envelope, encode_envelope, WireError, MAX_ID_BYTES};

/// The `MAX_PAYLOAD` used when generating the corpus.
const MAX_PAYLOAD: u32 = 64;

// Embed every fixture at compile time so the test binary is self-contained.
macro_rules! corpus {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/wire-corpus/",
            $name
        ))
    };
}

fn golden_envelope() -> ConnectionEnvelope {
    ConnectionEnvelope {
        protocol_version: PROTOCOL_VERSION,
        plan_id: PlanId::from("plan-a"),
        connection_id: ConnectionId::from("conn-b"),
        sequence: 0x0102_0304_0506_0708,
        value_kind: KindId::from("test/value"),
        payload: alloc::vec![0x00, 0x01, 0x02, 0xff],
    }
}

extern crate alloc;

// ---------------------------------------------------------------------------
// Deterministic byte-level stability
// ---------------------------------------------------------------------------

#[test]
fn golden_vector_is_stable() {
    let bytes: &[u8] = corpus!("golden.bin");
    assert_eq!(
        bytes,
        b"CNDW\x01\x01\x00\x06\x00plan-a\x06\x00conn-b\x08\x07\x06\x05\x04\x03\x02\x01\x0a\x00test/value\x04\x00\x00\x00\x00\x01\x02\xff"
    );
}

#[test]
fn golden_vector_round_trips() {
    let bytes: &[u8] = corpus!("golden.bin");
    let env = decode_envelope(bytes, MAX_PAYLOAD).expect("golden.bin must decode");
    assert_eq!(env, golden_envelope());
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"),
        bytes
    );
}

#[test]
fn empty_payload_vector_round_trips() {
    let bytes: &[u8] = corpus!("empty_payload.bin");
    let env = decode_envelope(bytes, MAX_PAYLOAD).expect("empty_payload.bin must decode");
    assert!(env.payload.is_empty());
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"),
        bytes
    );
}

#[test]
fn max_payload_vector_round_trips() {
    let bytes: &[u8] = corpus!("max_payload.bin");
    let env = decode_envelope(bytes, MAX_PAYLOAD).expect("max_payload.bin must decode");
    assert_eq!(env.payload.len(), MAX_PAYLOAD as usize);
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"),
        bytes
    );
}

#[test]
fn zero_capacity_ids_vector_round_trips() {
    let bytes: &[u8] = corpus!("zero_capacity_ids.bin");
    let env = decode_envelope(bytes, MAX_PAYLOAD).expect("zero_capacity_ids.bin must decode");
    assert_eq!(env.plan_id.as_str(), "");
    assert_eq!(env.connection_id.as_str(), "");
    assert_eq!(env.value_kind.as_str(), "");
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"),
        bytes
    );
}

#[test]
fn max_id_vector_round_trips() {
    let bytes: &[u8] = corpus!("max_id.bin");
    let env = decode_envelope(bytes, MAX_PAYLOAD).expect("max_id.bin must decode");
    assert_eq!(env.plan_id.as_str().len(), MAX_ID_BYTES);
    assert_eq!(env.connection_id.as_str().len(), MAX_ID_BYTES);
    assert_eq!(env.value_kind.as_str().len(), MAX_ID_BYTES);
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"),
        bytes
    );
}

// ---------------------------------------------------------------------------
// Identity-mutation vectors — valid frames with mutated identifiers
// ---------------------------------------------------------------------------

#[test]
fn wrong_plan_vector_decodes_to_mutated_plan() {
    let bytes: &[u8] = corpus!("wrong_plan.bin");
    let env = decode_envelope(bytes, MAX_PAYLOAD).expect("wrong_plan.bin must decode");
    assert_eq!(env.plan_id.as_str(), "other-plan");
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"),
        bytes
    );
}

#[test]
fn wrong_connection_vector_decodes_to_mutated_connection() {
    let bytes: &[u8] = corpus!("wrong_connection.bin");
    let env = decode_envelope(bytes, MAX_PAYLOAD).expect("wrong_connection.bin must decode");
    assert_eq!(env.connection_id.as_str(), "other-conn");
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"),
        bytes
    );
}

#[test]
fn wrong_kind_vector_decodes_to_mutated_kind() {
    let bytes: &[u8] = corpus!("wrong_kind.bin");
    let env = decode_envelope(bytes, MAX_PAYLOAD).expect("wrong_kind.bin must decode");
    assert_eq!(env.value_kind.as_str(), "other/kind");
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"),
        bytes
    );
}

#[test]
fn wrong_sequence_vector_decodes_and_re_encodes_identically() {
    let bytes: &[u8] = corpus!("wrong_sequence.bin");
    let env = decode_envelope(bytes, MAX_PAYLOAD).expect("wrong_sequence.bin must decode");
    assert_eq!(env.sequence, 0xDEAD_BEEF_CAFE_BABE_u64);
    assert_eq!(
        encode_envelope(&env, MAX_PAYLOAD).expect("re-encode"),
        bytes
    );
}

// ---------------------------------------------------------------------------
// Adversarial vectors — each must be rejected
// ---------------------------------------------------------------------------

#[test]
fn wrong_magic_vector_is_rejected() {
    assert_eq!(
        decode_envelope(corpus!("wrong_magic.bin"), MAX_PAYLOAD),
        Err(WireError::InvalidMagic)
    );
}

#[test]
fn wrong_wire_version_vector_is_rejected() {
    assert_eq!(
        decode_envelope(corpus!("wrong_wire_version.bin"), MAX_PAYLOAD),
        Err(WireError::UnsupportedWireFormat)
    );
}

#[test]
fn wrong_protocol_version_vector_is_rejected() {
    assert_eq!(
        decode_envelope(corpus!("wrong_protocol_version.bin"), MAX_PAYLOAD),
        Err(WireError::WrongProtocolVersion)
    );
}

#[test]
fn oversized_frame_vector_is_rejected() {
    assert_eq!(
        decode_envelope(corpus!("oversized_frame.bin"), MAX_PAYLOAD),
        Err(WireError::OversizedFrame)
    );
}

#[test]
fn oversized_payload_vector_is_rejected() {
    assert_eq!(
        decode_envelope(corpus!("oversized_payload.bin"), MAX_PAYLOAD),
        Err(WireError::OversizedPayload)
    );
}

#[test]
fn trailing_bytes_vector_is_rejected() {
    assert_eq!(
        decode_envelope(corpus!("trailing_bytes.bin"), MAX_PAYLOAD),
        Err(WireError::TrailingGarbage)
    );
}

#[test]
fn non_utf8_plan_vector_is_rejected() {
    assert_eq!(
        decode_envelope(corpus!("non_utf8_plan.bin"), MAX_PAYLOAD),
        Err(WireError::InvalidIdentifierEncoding)
    );
}

#[test]
fn id_length_overflow_vector_is_rejected() {
    assert_eq!(
        decode_envelope(corpus!("id_length_overflow.bin"), MAX_PAYLOAD),
        Err(WireError::IdentifierTooLong)
    );
}
