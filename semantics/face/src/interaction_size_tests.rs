use super::*;
use alloc::vec;

fn value(arguments: Vec<FaceInteractionArgument>) -> FaceInteraction {
    FaceInteraction {
        identity: FaceInteractionId("a".repeat(64)),
        face_id: "face/α".into(),
        face_revision: 0x0102030405060708,
        show_id: "show/🦀".into(),
        action_id: "send".into(),
        target: "target".into(),
        arguments,
        sequence: 0x1112131415161718,
    }
}
fn size(value: &FaceInteraction) -> Result<usize, FaceInteractionRefusal> {
    interaction_encoded_len(
        [
            64,
            value.face_id.len(),
            value.show_id.len(),
            value.action_id.len(),
            value.target.len(),
        ],
        &value.arguments,
    )
}
#[test]
fn exact_size_matches_wire_for_utf8_and_raw_arguments() {
    for arguments in [
        vec![],
        vec![
            FaceInteractionArgument {
                name: "名".into(),
                value_kind: "raw/α".into(),
                value: vec![0, 255, 128, 13],
            },
            FaceInteractionArgument {
                name: "other".into(),
                value_kind: "raw".into(),
                value: vec![],
            },
        ],
    ] {
        let value = value(arguments);
        assert_eq!(size(&value), Ok(value.encode().len()));
        assert_eq!(FaceInteraction::decode(&value.encode()).unwrap(), value);
    }
}
#[test]
fn exact_limit_and_one_byte_excess_match_encoder() {
    let mut value = value(vec![FaceInteractionArgument {
        name: "x".into(),
        value_kind: "raw".into(),
        value: vec![],
    }]);
    let overhead = value.encode().len();
    value.arguments[0]
        .value
        .resize(MAX_FACE_INTERACTION_BYTES - overhead, 255);
    assert_eq!(size(&value), Ok(MAX_FACE_INTERACTION_BYTES));
    assert_eq!(value.encode().len(), MAX_FACE_INTERACTION_BYTES);
    value.arguments[0].value.push(0);
    assert_eq!(size(&value), Err(FaceInteractionRefusal::OversizeValue));
    assert_eq!(value.encode().len(), MAX_FACE_INTERACTION_BYTES + 1);
}
#[test]
fn synthetic_length_overflow_refuses_without_large_allocations() {
    assert_eq!(
        encoded_len_from_lengths([usize::MAX]),
        Err(FaceInteractionRefusal::OversizeValue)
    );
    assert_eq!(
        encoded_len_from_lengths([usize::MAX - 25]),
        Err(FaceInteractionRefusal::OversizeValue)
    );
}
#[test]
fn digest_and_wire_keep_exact_known_byte_stream() {
    let mut value = value(vec![FaceInteractionArgument {
        name: "x".into(),
        value_kind: "raw".into(),
        value: vec![0, 255, 128, 13],
    }]);
    // Independent literal fixture: SHA-256 domain, byte-prefixed fields, LE integers,
    // argument count, byte-prefixed argument names/kinds, length and raw-value digest.
    assert_eq!(
        hex(&value.identity_digest()),
        "3a6917ec8b93777bc416120153731d62c3f1df0434f4bdb3e4163c9be66a25a3"
    );
    value.identity = value.derived_identity();
    assert!(identity_matches(&value.identity, &value.identity_digest()));
    let mut wrong = value.identity.clone();
    wrong.0.make_ascii_uppercase();
    assert!(!identity_matches(&wrong, &value.identity_digest()));
    for length in [0, 63, 65, 8193] {
        assert!(!identity_matches(
            &FaceInteractionId("a".repeat(length)),
            &value.identity_digest()
        ));
    }
    let prior = value.identity_digest();
    value.sequence += 1;
    assert_ne!(value.identity_digest(), prior);
    let wire = value.encode();
    assert_eq!(&wire[..4], b"CFI2");
    assert_eq!(&wire[4..8], &64u32.to_le_bytes());
    let integers = 4 + [
        value.identity.as_str(),
        &value.face_id,
        &value.show_id,
        &value.action_id,
        &value.target,
    ]
    .iter()
    .map(|s| 4 + s.len())
    .sum::<usize>();
    assert_eq!(
        &wire[integers..integers + 8],
        &value.face_revision.to_le_bytes()
    );
    assert_eq!(
        &wire[integers + 8..integers + 16],
        &value.sequence.to_le_bytes()
    );
}
