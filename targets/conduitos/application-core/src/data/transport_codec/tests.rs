//! Private wire conformance, exact allocation hints, and pre-visitor refusal.
use super::*;
use alloc::{collections::BTreeMap, format, string::String, vec, vec::Vec};
use core::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};
use serde::{
    Deserialize, Deserializer, Serializer,
    de::{SeqAccess, Visitor},
    ser::SerializeSeq,
};

fn encoded<T: Serialize + ?Sized>(value: &T) -> Vec<u8> {
    let mut output = vec![0; MAX_BYTES];
    let length = encode_to_slice(value, &mut output).unwrap();
    output.truncate(length);
    output
}
fn round_trip<T: Serialize + DeserializeOwned + PartialEq + fmt::Debug>(value: &T) {
    assert_eq!(&decode::<T>(&encoded(value)).unwrap(), value);
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Mixed {
    Unit,
    Newtype(Vec<u8>),
    Tuple(u32, Option<Vec<u8>>),
    Struct {
        values: BTreeMap<String, Vec<Option<u16>>>,
    },
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TupleRecord(u64, Vec<Vec<u8>>);
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct NewtypeRecord(Vec<Vec<u8>>);

#[test]
fn mixed_enums_structs_maps_options_tuples_and_arrays_round_trip() {
    for value in [
        Mixed::Unit,
        Mixed::Newtype(vec![1, 2, 3]),
        Mixed::Tuple(7, Some(vec![4, 5])),
        Mixed::Tuple(8, None),
        Mixed::Struct {
            values: BTreeMap::from([("$conduit_seq".into(), vec![Some(3), None])]),
        },
    ] {
        round_trip(&value);
    }
    round_trip(&TupleRecord(u64::MAX, vec![vec![], vec![1, 2]]));
    round_trip(&NewtypeRecord(vec![vec![7], vec![]]));
    round_trip(&([1u8, 2, 3], (), Some([4u16, 5])));
    round_trip(&String::from("unicode λ\nquote\"slash\\"));
}

#[test]
fn ordinary_large_byte_vector_gets_its_exact_initial_reservation() {
    let value = (0..129_539)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let bytes = encoded(&value);
    let decoded: Vec<u8> = decode(&bytes).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(
        decoded.capacity(),
        decoded.len(),
        "no geometric growth or trailing capacity"
    );
}

#[derive(Debug, PartialEq)]
struct ByteField(Vec<u8>);
impl Serialize for ByteField {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}
impl<'de> Deserialize<'de> for ByteField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Bytes;
        impl<'de> Visitor<'de> for Bytes {
            type Value = ByteField;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("bytes")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut value = Vec::with_capacity(sequence.size_hint().unwrap());
                while let Some(byte) = sequence.next_element()? {
                    value.push(byte);
                }
                Ok(ByteField(value))
            }
        }
        deserializer.deserialize_byte_buf(Bytes)
    }
}
#[test]
fn serialize_bytes_uses_the_same_verified_array_contract() {
    round_trip(&Some(ByteField(vec![0, 1, 254, 255])));
    assert_eq!(
        encoded(&vec![0u8, 1, 254, 255]),
        encoded(&ByteField(vec![0, 1, 254, 255]))
    );
}

static TYPED_VISITOR_CALLED: AtomicBool = AtomicBool::new(false);
struct Probe;
impl<'de> Deserialize<'de> for Probe {
    fn deserialize<D: Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
        TYPED_VISITOR_CALLED.store(true, Ordering::SeqCst);
        Err(serde::de::Error::custom("probe must not run"))
    }
}
#[test]
fn malformed_counts_versions_json_and_extents_refuse_before_typed_decode() {
    for bytes in [
        br#"[2,"CAPSEQ0",[0]]"#.as_slice(),
        br#"[2,"CAPSEQ1",[2,1]]"#,
        br#"[2,"CAPSEQ1",[1,1,2]]"#,
        br#"[2,"CAPSEQ1",[-1]]"#,
        br#"[2,"CAPSEQ1",[01,1]]"#,
        br#"[2,"CAPSEQ1",[1.0,1]]"#,
        br#"[2,"CAPSEQ1",[18446744073709551616]]"#,
        br#"[2,"CAPSEQ1",[0,]]"#,
        br#"[2,"CAPSEQ1",[0]]null"#,
        br#"[2,"CAPSEQ1",{"x":"\q"}]"#,
        br#"[2,"CAPSEQ1",{"x":"\ud800"}]"#,
        br#"[2,"CAPSEQ1",{"x":01}]"#,
        br#"[2,"CAPSEQ1",{"x":1e}]"#,
        br#"[2,"CAPSEQ1",{"x":NaN}]"#,
        br#"[2,"CAPSEQ1",{"x":[0],}]"#,
        br#"[2,"CAPSEQ1",{"x":false,"nested":[3,1,2]}]"#,
        br#"{"ordinary":"legacy public JSON"}"#,
    ] {
        TYPED_VISITOR_CALLED.store(false, Ordering::SeqCst);
        assert!(decode::<Probe>(bytes).is_err(), "{bytes:?}");
        assert!(!TYPED_VISITOR_CALLED.load(Ordering::SeqCst));
    }
    assert!(decode::<Probe>(&[b'[', b'2', b',', b'"', 0xff]).is_err());
}

#[test]
fn byte_and_depth_limits_are_finite_without_narrowing_the_json_profile() {
    let mut depth = String::from("0");
    for _ in 0..127 {
        depth = format!("[1,{depth}]");
    }
    // Outer version envelope is the 128th container.
    assert!(preflight::validate(format!("[2,\"CAPSEQ1\",{depth}]").as_bytes()).is_ok());
    depth = format!("[1,{depth}]");
    assert_eq!(
        preflight::validate(format!("[2,\"CAPSEQ1\",{depth}]").as_bytes()),
        Err(Refusal::Bounds)
    );
    let mut at_limit = vec![b' '; MAX_BYTES];
    at_limit[..1].copy_from_slice(b"0");
    assert!(preflight::validate(&at_limit).is_ok());
    at_limit.push(b' ');
    assert_eq!(preflight::validate(&at_limit), Err(Refusal::Bounds));
    let mut too_short = [0; 8];
    assert_eq!(
        encode_to_slice(&vec![1u8], &mut too_short),
        Err(Refusal::Capacity)
    );
}

struct UnknownLength;
impl Serialize for UnknownLength {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_seq(None)?.end()
    }
}
struct LyingLength(usize);
impl Serialize for LyingLength {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0))?;
        sequence.serialize_element(&7u8)?;
        sequence.end()
    }
}
#[test]
fn unknown_or_false_serialize_lengths_never_produce_an_admitted_frame() {
    let mut output = vec![0; 1024];
    assert!(encode_to_slice(&UnknownLength, &mut output).is_err());
    assert!(encode_to_slice(&LyingLength(0), &mut output).is_err());
    assert!(encode_to_slice(&LyingLength(2), &mut output).is_err());
}

#[test]
fn tuple_arity_is_checked_independently_of_valid_sequence_framing() {
    assert!(decode::<(u8, u8)>(&encoded(&vec![1u8])).is_err());
    assert!(decode::<[u8; 2]>(&encoded(&vec![1u8, 2, 3])).is_err());
}
