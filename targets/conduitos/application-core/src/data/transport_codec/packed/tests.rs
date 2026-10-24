use super::super::*;
use alloc::{format, string::String, vec, vec::Vec};
use serde::{Deserialize, Serialize};
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Blob {
    canonical_value: Vec<u8>,
}
fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    let mut out = vec![0; MAX_BYTES];
    let len = encode_to_slice(value, &mut out).unwrap();
    out.truncate(len);
    out
}
#[test]
fn all_byte_extrema_padding_and_exact_reservation_round_trip() {
    for length in 0..=258 {
        let value = Blob {
            canonical_value: (0..length).map(|n| n as u8).collect(),
        };
        let out = encode(&value);
        assert!(out.starts_with(COMPACT_PREFIX));
        assert!(out.windows(5).any(|s| s == b"~b64:"));
        let decoded: Blob = decode(&out).unwrap();
        assert_eq!(decoded.canonical_value.capacity(), length);
        assert_eq!(decoded, value);
    }
}
#[test]
#[allow(dead_code)] // Negative-only destination types must never successfully decode.
fn packed_permission_is_field_scoped_and_u8_only() {
    #[derive(Deserialize)]
    struct Wide {
        canonical_value: Vec<u64>,
    }
    #[derive(Deserialize)]
    struct Signed {
        canonical_value: Vec<i64>,
    }
    #[derive(Deserialize)]
    struct Flags {
        canonical_value: Vec<bool>,
    }
    #[derive(Deserialize)]
    struct Array {
        canonical_value: [u8; 1],
    }
    #[derive(Deserialize)]
    struct Other {
        bytes: Vec<u8>,
    }
    let input = br#"[2,"CAPSEQ2",{"canonical_value":"~b64:AA=="}]"#;
    assert!(decode::<Wide>(input).is_err());
    assert!(decode::<Signed>(input).is_err());
    assert!(decode::<Flags>(input).is_err());
    assert!(decode::<Array>(input).is_err());
    assert!(decode::<Other>(br#"[2,"CAPSEQ2",{"bytes":"~b64:AA=="}]"#).is_err());
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Siblings {
        canonical_value: Vec<u8>,
        other: Vec<u8>,
    }
    let siblings = Siblings {
        canonical_value: vec![255],
        other: vec![1, 2],
    };
    assert_eq!(decode::<Siblings>(&encode(&siblings)).unwrap(), siblings);
    assert!(
        decode::<Siblings>(br#"[2,"CAPSEQ2",{"canonical_value":"~b64:/w==","other":"~b64:AA=="}]"#)
            .is_err()
    );
    // Serializer rejects a future same-name non-byte field rather than corrupting it.
    #[derive(Serialize)]
    struct Wrong {
        canonical_value: Vec<u16>,
    }
    assert!(
        encode_to_slice(
            &Wrong {
                canonical_value: vec![1]
            },
            &mut vec![0; 1024]
        )
        .is_err()
    );
}
#[test]
fn legacy_arrays_and_manual_oracle_stay_v1_only() {
    let array = br#"[2,"CAPSEQ1",{"canonical_value":[2,0,255]}]"#;
    assert_eq!(decode::<Blob>(array).unwrap().canonical_value, vec![0, 255]);
    assert!(decode::<Blob>(br#"[2,"CAPSEQ1",{"canonical_value":"~b64:AP8="}]"#).is_err());
    assert!(decode::<Blob>(br#"[2,"CAPSEQ2",{"canonical_value":[2,0,255]}]"#).is_err());
    let mut legacy = vec![0; 1024];
    let length = encode_unvalidated(
        &Blob {
            canonical_value: vec![0, 255],
        },
        &mut legacy,
    )
    .unwrap();
    assert_eq!(&legacy[..length], array);
}
#[test]
fn ordinary_escaped_text_and_reserved_prefix_are_not_packed() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Text {
        label: String,
    }
    let value = Text {
        label: "~b64:not base64\n\"λ\\".into(),
    };
    assert_eq!(decode::<Text>(&encode(&value)).unwrap(), value);
    assert_eq!(
        decode::<String>(&encode(&value.label)).unwrap(),
        value.label
    );
}
#[test]
fn malformed_escaped_and_noncanonical_packing_refuse_before_typed_visitor() {
    struct Never;
    impl<'de> Deserialize<'de> for Never {
        fn deserialize<D: serde::Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
            panic!("typed visitor must not run")
        }
    }
    for value in [
        "",
        "~b64:A",
        "~b64:AAAA=",
        "~b64:AA=A",
        "~b64:=AAA",
        "~b64:AB==",
        "~b64:AAB=",
        "~b64:AA==AAAA",
        "~b64:AA-_",
        "~b64:λAAA",
        "~b64:AA\\u003d=",
        "\\u007eb64:AA==",
        "~b64:AA==\\n",
    ] {
        let input = format!(r#"[2,"CAPSEQ2",{{"canonical_value":"{value}"}}]"#);
        assert!(decode::<Never>(input.as_bytes()).is_err(), "{value}");
    }
    // Escaped key spelling receives the same no-scratch validation policy.
    assert!(
        decode::<Never>(br#"[2,"CAPSEQ2",{"canonical_\u0076alue":"\u007eb64:AA=="}]"#).is_err()
    );
    assert_eq!(
        decode::<Blob>(br#"[2,"CAPSEQ2",{"canonical_\u0076alue":"~b64:AA=="}]"#)
            .unwrap()
            .canonical_value,
        vec![0]
    );
}
#[test]
fn canonical_bound_checked_before_hint_and_output_reservation() {
    let value = Blob {
        canonical_value: vec![0; super::MAX_BYTES],
    };
    assert_eq!(decode::<Blob>(&encode(&value)).unwrap(), value);
    let too_large = Blob {
        canonical_value: vec![0; super::MAX_BYTES + 1],
    };
    assert!(encode_to_slice(&too_large, &mut vec![0; MAX_BYTES]).is_err());
    let encoded_len = (super::MAX_BYTES + 1).div_ceil(3) * 4;
    let payload = "A".repeat(encoded_len);
    let input = format!(r#"[2,"CAPSEQ2",{{"canonical_value":"~b64:{payload}"}}]"#);
    struct Never;
    impl<'de> Deserialize<'de> for Never {
        fn deserialize<D: serde::Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
            panic!("overbound reached typed visitor")
        }
    }
    assert!(input.len() < MAX_BYTES);
    assert!(decode::<Never>(input.as_bytes()).is_err());
}
