use super::*;
use alloc::{collections::BTreeMap, string::ToString, vec, vec::Vec};
use serde::{
    ser::{SerializeMap, Serializer},
    Serialize,
};
fn decode_output(encoded: &str) -> Vec<u8> {
    super::super::decode_base64(encoded).unwrap()
}
fn parity(value: &impl Serialize) {
    let expected = serde_json::to_vec(value).unwrap();
    let (base64, len) = encode(
        value,
        super::super::MAX_EXPANDED_BYTES,
        super::super::MAX_COMPRESSED_BYTES,
    )
    .unwrap();
    assert_eq!(len, expected.len());
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib(&decode_output(&base64)).unwrap(),
        expected
    );
}
#[derive(Serialize)]
enum Example {
    Unit,
    Newtype(Option<u64>),
    Tuple(bool, String),
    Struct { key: f64, nested: Vec<Option<i32>> },
}
#[derive(Serialize)]
struct Tuple(u8, String);
#[derive(Serialize)]
struct Newtype(String);
#[derive(Serialize)]
struct Unit;
struct Bytes(Vec<u8>);
impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}
struct Collected;
impl core::fmt::Display for Collected {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "é\0\\\"{}", 18446744073709551615_u64)
    }
}
impl Serialize for Collected {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
#[test]
fn streaming_json_matches_canonical_serde_json_compounds_strings_and_all_numeric_leaves() {
    parity(&Example::Unit);
    parity(&Example::Newtype(Some(u64::MAX)));
    parity(&Example::Tuple(true, "é\0\\\"🦀".into()));
    parity(&Example::Struct {
        key: -0.0,
        nested: vec![Some(i32::MIN), None, Some(i32::MAX)],
    });
    parity(&Tuple(255, "tuple".into()));
    parity(&Newtype("newtype".into()));
    parity(&Unit);
    parity(&[true, false]);
    parity(&None::<bool>);
    parity(&Some(true));
    parity(&());
    parity(&[i8::MIN, i8::MAX]);
    parity(&[i16::MIN, i16::MAX]);
    parity(&[i32::MIN, i32::MAX]);
    parity(&[i64::MIN, i64::MAX]);
    parity(&[i128::MIN, i128::MAX]);
    parity(&[u8::MIN, u8::MAX]);
    parity(&[u16::MIN, u16::MAX]);
    parity(&[u32::MIN, u32::MAX]);
    parity(&[u64::MIN, u64::MAX]);
    parity(&[u128::MIN, u128::MAX]);
    parity(&[
        f32::MIN,
        f32::MAX,
        f32::MIN_POSITIVE,
        -0.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ]);
    parity(&[
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        -0.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ]);
    let controls: String = (0..=31).map(char::from).collect();
    parity(&(controls, "é🦀\u{2028}\u{2029}/\\\"", '\0', '🦀'));
    parity(&Bytes((0..=255).collect()));
    parity(&Collected);
    parity(&BTreeMap::from([
        ("escaped\"key", vec![1, 2, 3]),
        ("key", vec![]),
    ]));
    parity(&BTreeMap::from([(i64::MIN, true), (i64::MAX, false)]));
    parity(&BTreeMap::from([(false, "false"), (true, "true")]));
    parity(&BTreeMap::from([('é', 1), ('🦀', 2)]));
    // Canonical human-readable configuration tags and large integer adapters.
    parity(&vec![
        conduit_core::ConfigurationValue::Text("linked\0program".into()),
        conduit_core::ConfigurationValue::U64(u64::MAX),
        conduit_core::ConfigurationValue::I64(i64::MIN),
        conduit_core::ConfigurationValue::Bool(true),
    ]);
}
#[test]
fn deflate_matches_exact_input_across_window_wrap_and_arbitrary_stream_chunking() {
    let mut random = 0x12345678_u32;
    let noise: Vec<_> = (0..12000)
        .map(|_| {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            random as u8
        })
        .collect();
    let window_edge = noise[..8192].repeat(3);
    let cases = [
        vec![0],
        vec![255; 9000],
        (0..=255).cycle().take(20000).collect(),
        noise,
        window_edge,
        b"complete Plan with repeated typed configuration".repeat(4000),
    ];
    for bytes in cases {
        let mut previous = None;
        for chunk in [1, 2, 7, 257, 258, 4097] {
            let mut encoder = deflate::Encoder::new(256 * 1024, 64 * 1024).unwrap();
            for part in bytes.chunks(chunk) {
                encoder.write(part).unwrap();
            }
            let (base64, len, _) = encoder.finish().unwrap();
            assert_eq!(len, bytes.len());
            assert_eq!(
                miniz_oxide::inflate::decompress_to_vec_zlib(&decode_output(&base64)).unwrap(),
                bytes
            );
            if let Some(expected) = &previous {
                assert_eq!(&base64, expected);
            }
            previous = Some(base64);
        }
    }
}
struct MissingValue;
impl Serialize for MissingValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_key("key")?;
        map.end()
    }
}
struct MissingKey;
impl Serialize for MissingKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_value("value")?;
        map.end()
    }
}
#[test]
fn stream_bounds_invalid_map_keys_and_incomplete_serialization_refuse() {
    assert_eq!(
        encode(&"oversized", 2, 64 * 1024),
        Err(CodecError::Capacity)
    );
    assert_eq!(
        encode(&"compressed bound", 256 * 1024, 2),
        Err(CodecError::Capacity)
    );
    assert!(encode(&BTreeMap::from([(vec![1, 2], "invalid key")]), 4096, 4096).is_err());
    assert!(encode(&MissingValue, 4096, 4096).is_err());
    assert!(encode(&MissingKey, 4096, 4096).is_err());
    // Adler checksum and exact consumed-stream validation remain decoder gates.
    let (base64, _) = encode(&"payload", 4096, 4096).unwrap();
    let mut compressed = decode_output(&base64);
    let last = compressed.len() - 1;
    compressed[last] ^= 1;
    assert!(miniz_oxide::inflate::decompress_to_vec_zlib(&compressed).is_err());
    compressed.truncate(last);
    assert!(miniz_oxide::inflate::decompress_to_vec_zlib(&compressed).is_err());
    assert!(!CodecError::Invalid.to_string().is_empty());
}

#[test]
fn changed_canonical_document_between_passes_refuses_before_publication() {
    struct Changing(core::cell::Cell<u64>);
    impl Serialize for Changing {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let value = self.0.get();
            self.0.set(value + 1);
            serializer.serialize_u64(value)
        }
    }
    assert_eq!(
        encode(&Changing(core::cell::Cell::new(100)), 4096, 4096),
        Err(CodecError::Invalid)
    );
}

#[test]
fn bounded_derived_tree_is_complete_and_length_limited() {
    for histogram in [
        alloc::vec![1u32;286],
        (0..286)
            .map(|n| if n < 30 { 1u32 << (n / 2) } else { 1 })
            .collect(),
    ] {
        let tree = super::huffman::Tree::from_histogram(&histogram);
        assert!(tree
            .lengths
            .iter()
            .all(|&length| (1..=15).contains(&length)));
        assert_eq!(
            tree.lengths
                .iter()
                .map(|&length| 1u32 << (15 - length))
                .sum::<u32>(),
            1 << 15
        );
    }
}

#[test]
fn base64_standard_vectors_and_noncanonical_padding_refuse() {
    for (encoded, plain) in [
        ("Zg==", "f"),
        ("Zm8=", "fo"),
        ("Zm9v", "foo"),
        ("Zm9vYg==", "foob"),
        ("Zm9vYmE=", "fooba"),
        ("Zm9vYmFy", "foobar"),
    ] {
        assert_eq!(
            super::super::decode_base64(encoded).unwrap(),
            plain.as_bytes()
        );
    }
    for encoded in [
        "", "Zg", "Zg=", "Zg===", "Zh==", "Zm9=", "=m9v", "Zm=v", "Zm9v
", "Zm9v-AAA",
    ] {
        assert!(super::super::decode_base64(encoded).is_none(), "{encoded}");
    }
}
