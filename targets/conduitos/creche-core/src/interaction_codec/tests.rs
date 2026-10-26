use super::*;
use alloc::{string::String, vec, vec::Vec};
use core::fmt;
use serde::de::{self, Visitor};

fn reference<T: Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = vec![0; MAX_BLOB_BYTES as usize];
    let mut remaining = bytes.as_mut_slice();
    ciborium::ser::into_writer(value, &mut remaining).unwrap();
    let used = MAX_BLOB_BYTES as usize - remaining.len();
    bytes.truncate(used);
    bytes
}
fn wire(bytes: &[u8]) -> Vec<u8> {
    let mut out = PREFIX.to_vec();
    out.extend_from_slice(bytes);
    out
}
fn differential<T>(value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + fmt::Debug,
{
    let reference = reference(value);
    let old: T = ciborium::de::from_reader_with_recursion_limit(reference.as_slice(), 64).unwrap();
    let new: T = decode(&encode(value).unwrap()).unwrap();
    assert_eq!(old, *value);
    assert_eq!(new, old);
}
#[test]
fn byte_arrays_pack_only_exact_u8_and_keep_empty_arrays() {
    for values in [vec![], vec![0u8], vec![23, 24, 255], (0..=255).collect()] {
        differential(&values);
        let bytes = encode(&values).unwrap();
        if values.is_empty() {
            assert_eq!(&bytes[PREFIX.len()..], &[0x80]);
        } else {
            assert_eq!(bytes[PREFIX.len()] >> 5, 2);
        }
    }
    for len in [23, 24, 255, 256] {
        let value = vec![255u8; len];
        differential(&value);
        assert!(encode(&value).unwrap().len() < PREFIX.len() + reference(&value).len());
    }
    differential(&vec![vec![0u8, 255], vec![], vec![24]]);
    differential(&[0u8, 23, 24, 255]);
    differential(&(0u8, 23u8, 24u8, 255u8));
    for values in [vec![0u16, 23, 24, 255], vec![256u16]] {
        differential(&values);
        assert_eq!(encode(&values).unwrap()[PREFIX.len()] >> 5, 4);
    }
    differential(&vec![-1i8, 0, 127]);
    differential(&vec![0i16, 23, 24, 255, 256]);
    differential(&vec![0u64, 255, 256, u64::MAX]);
    differential(&(1u8, -1i8, 2.5f64));
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
enum Tagged {
    Value { bytes: Vec<u8> },
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
enum Untagged {
    Bytes(Vec<u8>),
    Text(String),
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Flatten {
    label: u8,
    #[serde(flatten)]
    value: Tagged,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum External {
    New(Vec<u8>),
    Pair(Vec<u8>, Vec<u8>),
    Fields { bytes: Vec<u8> },
}
#[test]
fn nested_content_and_enum_payloads_keep_integer_sequence_semantics() {
    differential(&Tagged::Value {
        bytes: vec![0, 24, 255],
    });
    differential(&Untagged::Bytes(vec![0, 24, 255]));
    differential(&Flatten {
        label: 7,
        value: Tagged::Value {
            bytes: vec![0, 255],
        },
    });
    differential(&External::New(vec![0, 24, 255]));
    differential(&External::Pair(vec![1, 2], vec![255]));
    differential(&External::Fields {
        bytes: vec![1, 255],
    });
    // Custom visitor sees the same u64 callback as original CBOR positives,
    // including through Content captured by an internally tagged enum.
    let bytes = wire(&[0x42, 0, 255]);
    assert_eq!(
        decode::<Vec<OnlyU64>>(&bytes).unwrap(),
        vec![OnlyU64(0), OnlyU64(255)]
    );
    assert!(decode::<Vec<i8>>(&bytes).is_err());
    assert!(decode::<Vec<f64>>(&bytes).is_err());
}
#[derive(Debug, PartialEq)]
struct OnlyU64(u64);
impl Serialize for OnlyU64 {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(self.0.try_into().unwrap())
    }
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
enum TypedContent {
    Values { bytes: Vec<OnlyU64> },
}
#[test]
fn packed_content_preserves_u64_any_dispatch() {
    differential(&TypedContent::Values {
        bytes: vec![OnlyU64(0), OnlyU64(255)],
    });
}

impl<'de> Deserialize<'de> for OnlyU64 {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Exact;
        impl Visitor<'_> for Exact {
            type Value = OnlyU64;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("u64 callback")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<OnlyU64, E> {
                Ok(OnlyU64(v))
            }
        }
        d.deserialize_any(Exact)
    }
}
#[test]
fn tuple_widths_and_wrong_types_are_refused() {
    for n in [1, 3] {
        let bytes = encode(&vec![7u8; n]).unwrap();
        assert!(decode::<[u8; 2]>(&bytes).is_err());
    }
    assert_eq!(
        decode::<[u8; 2]>(&encode(&vec![7u8; 2]).unwrap()).unwrap(),
        [7, 7]
    );
    assert!(decode::<[u16; 2]>(&wire(&[0x83, 1, 2, 3])).is_err());
    assert!(decode::<u8>(&wire(&[0x41, 1])).is_err());
    assert!(decode::<String>(&wire(&[0x41, 1])).is_err());
}
struct NativeBytes;
impl Serialize for NativeBytes {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(&[1, 2, 3])
    }
}
#[test]
fn source_native_bytes_and_tagged_large_integer_encodings_are_unsupported() {
    assert_eq!(encode(&NativeBytes), Err(Refusal::Capacity));
    assert_eq!(encode(&vec![NativeBytes]), Err(Refusal::Capacity));
    assert_eq!(encode(&(1u8, NativeBytes)), Err(Refusal::Capacity));
    assert_eq!(encode(&u128::MAX), Err(Refusal::Capacity));
    assert_eq!(encode(&i128::MIN), Err(Refusal::Capacity));
    differential(&(u64::MAX as u128));
    differential(&(-(u64::MAX as i128) - 1));
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Fields {
    bytes: Vec<u8>,
}
#[test]
fn map_field_presence_and_all_envelope_guards_remain_strict() {
    differential(&Fields {
        bytes: vec![0, 24, 255],
    });
    for raw in [
        &[0xa0][..],                   // missing
        &[0xa1, 0x61, b'x', 0x80][..], // unknown
        &[
            0xa2, 0x65, b'b', b'y', b't', b'e', b's', 0x80, 0x65, b'b', b'y', b't', b'e', b's',
            0x80,
        ][..],
    ] {
        assert_eq!(decode::<Fields>(&wire(raw)), Err(Refusal::Malformed));
    }
    assert_eq!(decode::<Vec<u8>>(&[]), Err(Refusal::Bounds));
    assert_eq!(
        decode::<Vec<u8>>(&vec![0; MAX_BLOB_BYTES as usize + 1]),
        Err(Refusal::Bounds)
    );
    assert_eq!(decode::<Vec<u8>>(b"CDCI\x01\x80"), Err(Refusal::Malformed));
    for raw in [&[0x42, 1][..], &[0x41, 1, 0][..], &[0xff][..]] {
        assert_eq!(decode::<Vec<u8>>(&wire(raw)), Err(Refusal::Malformed));
    }
    let exact = vec![0; MAX_BLOB_BYTES as usize];
    assert_eq!(decode::<Vec<u8>>(&exact), Err(Refusal::Malformed));
}
#[test]
fn packed_container_counts_toward_recursion_limit_even_through_ignored_any() {
    // 63 array parents plus a byte-array container is exactly 64. At 64
    // parents the synthetic container must not evade Ciborium's own guard.
    for (parents, accepted) in [(62, true), (63, true), (64, false), (65, false)] {
        let mut raw = vec![0x81; parents];
        raw.extend_from_slice(&[0x41, 1]);
        assert_eq!(
            decode::<de::IgnoredAny>(&wire(&raw)).is_ok(),
            accepted,
            "parents={parents}"
        );
    }
}

#[test]
fn exact_transfer_cap_is_accepted_and_next_byte_is_refused() {
    // Five prefix bytes plus the five-byte CBOR byte-string header.
    let value = vec![255u8; MAX_BLOB_BYTES as usize - PREFIX.len() - 5];
    let encoded = encode(&value).unwrap();
    assert_eq!(encoded.len(), MAX_BLOB_BYTES as usize);
    assert_eq!(decode::<Vec<u8>>(&encoded).unwrap(), value);
    let mut larger = value;
    larger.push(255);
    assert_eq!(encode(&larger), Err(Refusal::Capacity));
}
#[test]
fn signed_positive_sequences_are_not_reclassified_and_options_keep_dispatch() {
    let value = vec![0i8, 23, 127];
    differential(&value);
    assert_eq!(encode(&value).unwrap()[PREFIX.len()] >> 5, 4);
    assert_eq!(
        decode::<Vec<Option<OnlyU64>>>(&wire(&[0x42, 0, 255])).unwrap(),
        vec![Some(OnlyU64(0)), Some(OnlyU64(255))]
    );
}

#[derive(Serialize, Deserialize)]
enum Nested {
    Next(alloc::boxed::Box<Nested>),
    Bytes(Vec<u8>),
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
enum NestedContent {
    Next(alloc::boxed::Box<NestedContent>),
    Bytes(Vec<u8>),
}
#[test]
fn enum_seed_and_content_buffering_cannot_hide_the_packed_container_depth() {
    for (parents, accepted) in [(61, true), (62, true), (63, false)] {
        let mut value = Nested::Bytes(vec![255]);
        let mut content = NestedContent::Bytes(vec![255]);
        for _ in 0..parents {
            value = Nested::Next(alloc::boxed::Box::new(value));
            content = NestedContent::Next(alloc::boxed::Box::new(content));
        }
        assert_eq!(
            decode::<Nested>(&encode(&value).unwrap()).is_ok(),
            accepted,
            "enum parents={parents}"
        );
        assert_eq!(
            decode::<NestedContent>(&encode(&content).unwrap()).is_ok(),
            accepted,
            "Content parents={parents}"
        );
    }
}
