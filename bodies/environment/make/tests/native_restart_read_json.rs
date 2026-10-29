//! Owned flash-reader JSON parity; no MCU proof.
extern crate alloc;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_read_json.rs"]
mod json;
use serde::{Deserialize, Serialize};
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8], chunk: usize) -> Result<T, json::Error> {
    let mut position = 0;
    json::from_reader(
        |out| {
            let count = (bytes.len() - position).min(chunk).min(out.len());
            out[..count].copy_from_slice(&bytes[position..position + count]);
            position += count;
            Ok(count)
        },
        bytes.len(),
    )
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Document {
    name: String,
    maybe: Option<Vec<Choice>>,
    number: i128,
    unsigned: u128,
    map: std::collections::BTreeMap<u32, String>,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum Choice {
    Unit,
    New(String),
    Pair(u8, bool),
    Record { n: f64, nested: Option<Box<Choice>> },
}
#[test]
fn owned_full_struct_enum_and_numeric_parity_at_every_fragment_size() {
    let input = Document {
        name: "\"\\\n\t雪🦀".into(),
        maybe: Some(vec![
            Choice::Unit,
            Choice::New("text".repeat(180)),
            Choice::Pair(255, true),
            Choice::Record {
                n: -1.25e-30,
                nested: Some(Box::new(Choice::New("hi".into()))),
            },
        ]),
        number: i128::MIN,
        unsigned: u128::MAX,
        map: [(12, "twelve".into())].into(),
    };
    let bytes = serde_json::to_vec(&input).unwrap();
    for chunk in 1..=128 {
        assert_eq!(decode::<Document>(&bytes, chunk).unwrap(), input);
    }
}
#[test]
fn dynamic_owned_content_and_bad_json_match_reference() {
    for text in [
        "null",
        "true",
        "-1",
        "18446744073709551615",
        "1.25e20",
        "\"\\uD834\\uDD1E雪\"",
        "{\"a\":[1,false,null,{\"b\":\"ok\"}]}",
        "[]",
        "{}",
    ] {
        assert_eq!(
            decode::<serde_json::Value>(text.as_bytes(), 1).unwrap(),
            serde_json::from_str::<serde_json::Value>(text).unwrap()
        );
    }
    for text in [
        "",
        "[",
        "{",
        "[1,]",
        "{\"a\":1,}",
        "[1 2]",
        "{a:1}",
        "{\"a\" 1}",
        "truefalse",
        "01",
        "+1",
        "NaN",
        "1e9999",
        "\"\\uD800\"",
        "\"\\uDC00\"",
        "\"a\nb\"",
        "true false",
        "nullx",
        "\"\\x\"",
        "\"unterminated",
    ] {
        assert!(
            serde_json::from_str::<serde_json::Value>(text).is_err(),
            "reference {text}"
        );
        for chunk in [1, 17, 128] {
            assert!(
                decode::<serde_json::Value>(text.as_bytes(), chunk).is_err(),
                "stream {text}"
            );
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Strict {
    #[allow(dead_code)]
    hold: NullOnly,
}
#[derive(Debug)]
struct NullOnly;
impl<'de> Deserialize<'de> for NullOnly {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Null;
        impl<'de> serde::de::Visitor<'de> for Null {
            type Value = NullOnly;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("null")
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<NullOnly, E> {
                Ok(NullOnly)
            }
            fn visit_some<D: serde::Deserializer<'de>>(self, _: D) -> Result<NullOnly, D::Error> {
                Err(serde::de::Error::custom("refused"))
            }
        }
        deserializer.deserialize_option(Null)
    }
}
#[test]
fn strict_fields_and_null_only_refuse_before_unwanted_value_traversal() {
    assert!(decode::<Strict>(br#"{"hold":null}"#, 1).is_ok());
    for prefix in [br#"{"hold":{"# as &[u8], br#"{"unknown":{"#] {
        let mut consumed = 0;
        let result = json::from_reader::<Strict>(
            |out| {
                assert!(consumed < prefix.len(), "must not traverse refused subtree");
                out[0] = prefix[consumed];
                consumed += 1;
                Ok(1)
            },
            32608,
        );
        assert!(result.is_err());
        assert!(consumed <= prefix.len());
    }
}
#[test]
fn depth_truncation_limit_read_errors_and_extra_data_are_closed() {
    for depth in 0..=130 {
        let text = format!("{}null{}", "[".repeat(depth), "]".repeat(depth));
        assert_eq!(
            decode::<serde_json::Value>(text.as_bytes(), 3).is_ok(),
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "depth {depth}"
        );
    }
    let bytes = br#"{"name":"abc","maybe":null,"number":-12,"unsigned":12,"map":{"1":"x"}}"#;
    for end in 0..bytes.len() {
        assert!(decode::<Document>(&bytes[..end], 7).is_err());
    }
    for fault in 0..=bytes.len() {
        let mut cursor = 0;
        assert!(json::from_reader::<Document>(
            |out| {
                if cursor == fault {
                    return Err(());
                }
                out[0] = bytes[cursor];
                cursor += 1;
                Ok(1)
            },
            bytes.len()
        )
        .is_err());
    }
    assert!(json::from_reader::<bool>(|_| Ok(129), 32608).is_err());
    let mut once = false;
    assert!(json::from_reader::<bool>(
        |out| {
            if once {
                return Ok(0);
            }
            once = true;
            out[..5].copy_from_slice(b"true ");
            Ok(5)
        },
        4
    )
    .is_err());
    assert_eq!(decode::<bool>(b"true \t\r\n", 2), Ok(true));
}
#[test]
fn numeric_key_spelling_and_range_match_reference() {
    type Map = std::collections::BTreeMap<u64, bool>;
    for key in [
        "0",
        "-0",
        "01",
        "+1",
        "1.0",
        "1e0",
        "18446744073709551615",
        "18446744073709551616",
        " 1",
        "1 ",
        r"\u0031",
        "-1",
        "true",
    ] {
        let text = format!(r#"{{"{key}":true}}"#);
        let expected = serde_json::from_str::<Map>(&text);
        let actual = decode::<Map>(text.as_bytes(), 1);
        assert_eq!(actual.is_ok(), expected.is_ok(), "key {key}");
        if let (Ok(actual), Ok(expected)) = (actual, expected) {
            assert_eq!(actual, expected);
        }
    }
}
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Untagged {
    Text(String),
    Tuple(u8, bool),
    Object { name: String, value: Option<u64> },
}
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "kind")]
enum Internal {
    Value { text: String },
    Empty,
}
#[test]
fn serde_owned_content_for_untagged_and_internally_tagged_documents() {
    for text in [r#""text""#, "[12,true]", r#"{"name":"n","value":12}"#] {
        assert_eq!(
            decode::<Untagged>(text.as_bytes(), 1).unwrap(),
            serde_json::from_str::<Untagged>(text).unwrap()
        );
    }
    for text in [r#"{"kind":"Value","text":"x"}"#, r#"{"kind":"Empty"}"#] {
        assert_eq!(
            decode::<Internal>(text.as_bytes(), 1).unwrap(),
            serde_json::from_str::<Internal>(text).unwrap()
        );
    }
}

#[derive(Debug, PartialEq)]
struct Bytes<const BUFFER: bool>(Vec<u8>);
impl<'de, const BUFFER: bool> Deserialize<'de> for Bytes<BUFFER> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OwnedBytes<const BUFFER: bool>;
        impl<'de, const BUFFER: bool> serde::de::Visitor<'de> for OwnedBytes<BUFFER> {
            type Value = Bytes<BUFFER>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("owned bytes")
            }
            fn visit_bytes<E: serde::de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
                Ok(Bytes(value.to_vec()))
            }
            fn visit_byte_buf<E: serde::de::Error>(self, value: Vec<u8>) -> Result<Self::Value, E> {
                Ok(Bytes(value))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut result = Vec::new();
                while let Some(byte) = seq.next_element()? {
                    result.push(byte);
                }
                Ok(Bytes(result))
            }
        }
        if BUFFER {
            deserializer.deserialize_byte_buf(OwnedBytes)
        } else {
            deserializer.deserialize_bytes(OwnedBytes)
        }
    }
}
fn parity<T: serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(bytes: &[u8]) {
    for chunk in [1, 127, 128] {
        let expected = serde_json::from_slice::<T>(bytes);
        let actual = decode::<T>(bytes, chunk);
        assert_eq!(actual.is_ok(), expected.is_ok(), "{bytes:?} chunk{chunk}");
        if let (Ok(actual), Ok(expected)) = (actual, expected) {
            assert_eq!(actual, expected);
        }
    }
}
#[test]
fn byte_string_and_sequence_and_struct_representations_match_reference() {
    for bytes in [
        b"[0,127,255]" as &[u8],
        b"[]",
        b"[256]",
        b"[-1]",
        b"\"abc\\n\\u96ea\"",
        b"\"\xff\"",
        b"\"\\uD834\\uDD1E\"",
        b"null",
        b"[1,]",
    ] {
        parity::<Bytes<false>>(bytes);
        parity::<Bytes<true>>(bytes);
    }
    for bytes in [br#"["n",null,-12,12,{"1":"x"}]"# as &[u8], br#"{"name":"n","maybe":null,"number":-12,"unsigned":12,"map":{}}"#, br#"{"name":"a","name":"b","maybe":null,"number":0,"unsigned":0,"map":{}}"#, br#"{"name":"a","maybe":null,"number":0,"unsigned":0,"map":{},"ignored":{"nested":[1,true]}}"#, br#"["n",null,-12,12,{},false]"#, br#"["n",null,-12,12]"#] {
        parity::<Document>(bytes);
    }
    parity::<serde_json::Value>(br#"{"a":1,"a":2}"#);
}
#[test]
fn scalar_numeric_limits_exponents_and_owned_character_forwarding_match_reference() {
    for text in [
        "0",
        "-0",
        "0.0",
        "-0.0",
        "1.0",
        "1e0",
        "-1.25e-30",
        "1e-9999",
        "1e9999",
        "01",
        "+1",
        "1e",
        "1.",
        "1e+2",
        "255",
        "256",
        "-129",
        "-128",
        "18446744073709551615",
        "18446744073709551616",
        "170141183460469231731687303715884105727",
        "170141183460469231731687303715884105728",
        "-170141183460469231731687303715884105728",
        "-170141183460469231731687303715884105729",
        "340282366920938463463374607431768211455",
        "340282366920938463463374607431768211456",
    ] {
        let bytes = text.as_bytes();
        parity::<i8>(bytes);
        parity::<u8>(bytes);
        parity::<i64>(bytes);
        parity::<u64>(bytes);
        parity::<i128>(bytes);
        parity::<u128>(bytes);
        parity::<f32>(bytes);
        parity::<f64>(bytes);
    }
    for text in [
        r#""a""#,
        r#""雪""#,
        r#""\uD834\uDD1E""#,
        r#""ab""#,
        r#""""#,
        "null",
    ] {
        parity::<char>(text.as_bytes());
    }
}
#[test]
fn complete_payload_limit_allows_one_large_owned_token() {
    let text = format!("\"{}\"", "x".repeat(32606));
    assert_eq!(text.len(), 32608);
    let decoded = decode::<String>(text.as_bytes(), 128).unwrap();
    assert_eq!(decoded.len(), 32606);
}
