use super::*;
use alloc::{vec, vec::Vec};
use ciborium::Value;

#[test]
fn finite_cbor_and_mutated_frames_agree_with_independent_value_decoder() {
    let values = vec![
        Value::Null,
        Value::Bool(true),
        Value::Integer(u64::MAX.into()),
        Value::Integer(i64::MIN.into()),
        Value::Float(1.25),
        Value::Text("λ \" \n".into()),
        Value::Bytes(vec![0, 24, 255]),
        Value::Array(vec![Value::Bool(false), Value::Null]),
        Value::Map(vec![(Value::Text("key".into()), Value::Integer(42.into()))]),
    ];
    for value in values {
        let root = Value::Map(vec![(Value::Text("payload".into()), value)]);
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&root, &mut bytes).unwrap();
        assert_eq!(validate(&bytes, 256 * 1024), Ok(()));
        for offset in 0..bytes.len() {
            for byte in [0, 23, 24, 31, 64, 127, 128, 159, 255] {
                let mut changed = bytes.clone();
                changed[offset] = byte;
                if validate(&changed, 256 * 1024).is_ok() {
                    let mut remaining = changed.as_slice();
                    let _: Value = ciborium::de::from_reader(&mut remaining).unwrap();
                    assert!(remaining.is_empty());
                }
            }
        }
    }
}

#[test]
fn counts_indefinite_lengths_nonfinite_floats_and_big_tags_refuse() {
    for value in [
        &[0x9b, 255, 255, 255, 255, 255, 255, 255, 255][..],
        &[0x9f, 0xff],
        &[0x61, 0xff],
        &[0xf9, 0x7c, 0],
        &[0xfa, 0x7f, 0x80, 0, 0],
        &[0xfb, 0x7f, 0xf0, 0, 0, 0, 0, 0, 0],
        &[
            0xc2, 0x51, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        &[0xc2, 0],
        &[0xc4, 0x80],
    ] {
        let mut bytes = vec![0xa1, 0x61, b'x'];
        bytes.extend_from_slice(value);
        assert!(validate(&bytes, 256 * 1024).is_err(), "{value:x?}");
    }
}
