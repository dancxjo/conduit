use super::*;
use alloc::{string::String, vec, vec::Vec};
use serde::{Deserialize, Serialize};

#[test]
fn binary16_all_bit_patterns_match_independent_decoder_or_refuse_nonfinite() {
    for bits in 0..=u16::MAX {
        let [hi, lo] = bits.to_be_bytes();
        let bytes = [0xf9, hi, lo];
        let reference: f32 = ciborium::de::from_reader(bytes.as_slice()).unwrap();
        let actual = decode::<f32>(&bytes);
        if reference.is_finite() {
            assert_eq!(actual.unwrap().to_bits(), reference.to_bits());
        } else {
            assert!(actual.is_err());
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Choice {
    Unit,
    Newtype(u128),
    Tuple(i128, bool),
    Struct { name: String, bytes: Vec<u8> },
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Record {
    choice: Choice,
    optional: Option<Choice>,
    pair: (char, u64),
    floats: [f64; 3],
}
#[test]
fn nested_models_match_independent_decoder_and_reject_every_truncation() {
    for choice in [
        Choice::Unit,
        Choice::Newtype(u128::MAX),
        Choice::Tuple(i128::MIN, true),
        Choice::Struct {
            name: "λ \n".into(),
            bytes: vec![0, 23, 24, 255],
        },
    ] {
        let value = Record {
            choice,
            optional: Some(Choice::Unit),
            pair: ('λ', u64::MAX),
            floats: [0.0, -0.0, 1.25],
        };
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&value, &mut bytes).unwrap();
        assert_eq!(decode::<Record>(&bytes).unwrap(), value);
        let reference: Record = ciborium::de::from_reader(bytes.as_slice()).unwrap();
        assert_eq!(decode::<Record>(&bytes).unwrap(), reference);
        for length in 0..bytes.len() {
            assert!(decode::<Record>(&bytes[..length]).is_err());
        }
        bytes.push(0);
        assert!(decode::<Record>(&bytes).is_err());
    }
}

#[test]
fn preflight_and_typed_skip_share_the_exact_container_depth_bound() {
    for arrays in [63, 64] {
        let mut bytes = vec![0xa1, 0x61, b'x'];
        bytes.extend(core::iter::repeat_n(0x81, arrays));
        bytes.push(0);
        let checked = super::super::preflight::validate(&bytes, 256 * 1024);
        let decoded = decode::<serde::de::IgnoredAny>(&bytes);
        if arrays == 63 {
            assert!(checked.is_ok());
            assert!(decoded.is_ok());
        } else {
            assert!(checked.is_err());
            assert!(decoded.is_err());
        }
    }
}
