//! Compare the direct visitor against the original complete untagged entrance.
use conduit_core::ConfigurationValue;
use serde::{
    de::{value, IntoDeserializer},
    Deserialize, Deserializer,
};

const MAXIMUM: u64 = 9_007_199_254_740_991;

fn legacy_unsigned<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    use serde::de::Error as _;
    if !deserializer.is_human_readable() {
        return u64::deserialize(deserializer);
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum HumanInteger {
        Number(u64),
        Decimal(String),
    }
    match HumanInteger::deserialize(deserializer)? {
        HumanInteger::Number(value) if value <= MAXIMUM => Ok(value),
        HumanInteger::Number(value) => Err(D::Error::custom(format!("JSON integer {value} exceeds JavaScript's exact integer range; encode it as a decimal string"))),
        HumanInteger::Decimal(value) => value.parse().map_err(|_| D::Error::custom("invalid exact u64 decimal string")),
    }
}
fn legacy_signed<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    use serde::de::Error as _;
    if !deserializer.is_human_readable() {
        return i64::deserialize(deserializer);
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum HumanInteger {
        Number(i64),
        Decimal(String),
    }
    match HumanInteger::deserialize(deserializer)? {
        HumanInteger::Number(value) if (-(MAXIMUM as i64)..=MAXIMUM as i64).contains(&value) => Ok(value),
        HumanInteger::Number(value) => Err(D::Error::custom(format!("JSON integer {value} exceeds JavaScript's exact integer range; encode it as a decimal string"))),
        HumanInteger::Decimal(value) => value.parse().map_err(|_| D::Error::custom("invalid exact i64 decimal string")),
    }
}
#[derive(Deserialize)]
enum Legacy {
    Bool(bool),
    U64(#[serde(deserialize_with = "legacy_unsigned")] u64),
    I64(#[serde(deserialize_with = "legacy_signed")] i64),
}
impl Legacy {
    fn current(self) -> ConfigurationValue {
        match self {
            Self::Bool(v) => ConfigurationValue::Bool(v),
            Self::U64(v) => ConfigurationValue::U64(v),
            Self::I64(v) => ConfigurationValue::I64(v),
        }
    }
}
fn require_json_parity(variant: &str, token: &str) {
    let text = format!("{{\"{variant}\":{token}}}");
    let old = serde_json::from_str::<Legacy>(&text).map(Legacy::current);
    let new = serde_json::from_str::<ConfigurationValue>(&text);
    assert_eq!(old.as_ref().ok(), new.as_ref().ok(), "{text}");
    if let (Err(old), Err(new)) = (old, new) {
        let old = old.to_string();
        if old.starts_with("JSON integer ") || old.starts_with("invalid exact ") {
            assert_eq!(old, new.to_string(), "{text}");
        }
    }
}
#[test]
fn exact_integer_json_matches_legacy_acceptance_and_explicit_errors() {
    let tokens = [
        "0",
        "1",
        "-0",
        "-1",
        "9007199254740990",
        "9007199254740991",
        "9007199254740992",
        "-9007199254740991",
        "-9007199254740992",
        "18446744073709551615",
        "18446744073709551616",
        "9223372036854775807",
        "9223372036854775808",
        "-9223372036854775808",
        "-9223372036854775809",
        "1.0",
        "1e0",
        "1E+0",
        "-0.0",
        "1.5",
        "1e100",
        "true",
        "false",
        "null",
        "[]",
        "[1,[2]]",
        "{}",
        "{\"nested\":[1]}",
        "\"0\"",
        "\"+0\"",
        "\"-0\"",
        "\"000001\"",
        "\"+000001\"",
        "\"-000001\"",
        "\"18446744073709551615\"",
        "\"18446744073709551616\"",
        "\"-9223372036854775808\"",
        "\"-9223372036854775809\"",
        "\"9007199254740992\"",
        "\"-9007199254740992\"",
        "\"\"",
        "\"+\"",
        "\"-\"",
        "\" 1\"",
        "\"1 \"",
        "\"1.0\"",
        "\"1e0\"",
        "\"++1\"",
        "\"+-1\"",
        "\"☃\"",
        "\"\\u0030\\u0031\"",
        "\"\\u002b0001\"",
        "\"1\\n\"",
    ];
    for variant in ["U64", "I64"] {
        for token in tokens {
            require_json_parity(variant, token);
        }
    }
}

fn require_human_scalar_parity<'de, D>(variant: &'static str, old: D, new: D)
where
    D: IntoDeserializer<'de, value::Error>,
{
    let old_map: value::MapDeserializer<'de, _, value::Error> =
        value::MapDeserializer::new(core::iter::once((variant, old)));
    let new_map: value::MapDeserializer<'de, _, value::Error> =
        value::MapDeserializer::new(core::iter::once((variant, new)));
    let old = Legacy::deserialize(value::MapAccessDeserializer::new(old_map)).map(Legacy::current);
    let new = ConfigurationValue::deserialize(value::MapAccessDeserializer::new(new_map));
    assert_eq!(old.as_ref().ok(), new.as_ref().ok());
    assert_eq!(old.is_ok(), new.is_ok());
}
struct OwnedBytes(Vec<u8>);
impl<'de> Deserializer<'de> for OwnedBytes {
    type Error = value::Error;
    fn deserialize_any<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_byte_buf(self.0)
    }
    serde::forward_to_deserialize_any! { bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier ignored_any }
}
impl<'de> IntoDeserializer<'de, value::Error> for OwnedBytes {
    type Deserializer = Self;
    fn into_deserializer(self) -> Self {
        self
    }
}

#[test]
fn exact_integer_human_strings_bytes_and_scalar_categories_match_legacy() {
    for variant in ["U64", "I64"] {
        for text in [
            "+0001",
            "-0",
            "18446744073709551615",
            "-9223372036854775808",
            "",
            " 1",
        ] {
            require_human_scalar_parity(
                variant,
                value::StrDeserializer::<value::Error>::new(text),
                value::StrDeserializer::new(text),
            );
            require_human_scalar_parity(
                variant,
                value::BorrowedStrDeserializer::<value::Error>::new(text),
                value::BorrowedStrDeserializer::new(text),
            );
            require_human_scalar_parity(
                variant,
                value::StringDeserializer::<value::Error>::new(text.into()),
                value::StringDeserializer::new(text.into()),
            );
            require_human_scalar_parity(
                variant,
                OwnedBytes(text.as_bytes().to_vec()),
                OwnedBytes(text.as_bytes().to_vec()),
            );
            require_human_scalar_parity(
                variant,
                value::BytesDeserializer::<value::Error>::new(text.as_bytes()),
                value::BytesDeserializer::new(text.as_bytes()),
            );
            require_human_scalar_parity(
                variant,
                value::BorrowedBytesDeserializer::<value::Error>::new(text.as_bytes()),
                value::BorrowedBytesDeserializer::new(text.as_bytes()),
            );
        }
        require_human_scalar_parity(
            variant,
            value::BytesDeserializer::<value::Error>::new(&[0xff]),
            value::BytesDeserializer::new(&[0xff]),
        );
        require_human_scalar_parity(variant, OwnedBytes(vec![0xff]), OwnedBytes(vec![0xff]));
        require_human_scalar_parity(variant, '1', '1');
        require_human_scalar_parity(variant, 'x', 'x');
        require_human_scalar_parity(variant, '☃', '☃');
        require_human_scalar_parity(variant, true, true);
        require_human_scalar_parity(variant, false, false);
        require_human_scalar_parity(variant, 1.0f32, 1.0f32);
        require_human_scalar_parity(variant, 1.0f64, 1.0f64);
        require_human_scalar_parity(variant, -0.0f64, -0.0f64);
        require_human_scalar_parity(variant, 1i8, 1i8);
        require_human_scalar_parity(variant, i8::MIN, i8::MIN);
        require_human_scalar_parity(variant, 1u8, 1u8);
        require_human_scalar_parity(variant, u8::MAX, u8::MAX);
        require_human_scalar_parity(variant, -1i16, -1i16);
        require_human_scalar_parity(variant, i16::MAX, i16::MAX);
        require_human_scalar_parity(variant, 1u16, 1u16);
        require_human_scalar_parity(variant, u16::MAX, u16::MAX);
        require_human_scalar_parity(variant, -1i32, -1i32);
        require_human_scalar_parity(variant, i32::MAX, i32::MAX);
        require_human_scalar_parity(variant, 1u32, 1u32);
        require_human_scalar_parity(variant, u32::MAX, u32::MAX);
        require_human_scalar_parity(variant, 1i64, 1i64);
        require_human_scalar_parity(variant, i64::MAX, i64::MAX);
        require_human_scalar_parity(variant, 1u64, 1u64);
        require_human_scalar_parity(variant, u64::MAX, u64::MAX);
        require_human_scalar_parity(variant, i64::MIN, i64::MIN);
        require_human_scalar_parity(variant, 1i128, 1i128);
        require_human_scalar_parity(variant, i128::MIN, i128::MIN);
        require_human_scalar_parity(variant, 1u128, 1u128);
        require_human_scalar_parity(variant, u128::MAX, u128::MAX);
        require_human_scalar_parity(variant, 1isize, 1isize);
        require_human_scalar_parity(variant, isize::MIN, isize::MIN);
        require_human_scalar_parity(variant, 1usize, 1usize);
        require_human_scalar_parity(variant, usize::MAX, usize::MAX);
    }
}
#[test]
fn exact_integer_serialization_and_nonhuman_roundtrip_are_unchanged() {
    for value in [
        ConfigurationValue::U64(0),
        ConfigurationValue::U64(MAXIMUM),
        ConfigurationValue::U64(MAXIMUM + 1),
        ConfigurationValue::U64(u64::MAX),
        ConfigurationValue::I64(i64::MIN),
        ConfigurationValue::I64(-(MAXIMUM as i64)),
        ConfigurationValue::I64(MAXIMUM as i64 + 1),
        ConfigurationValue::I64(i64::MAX),
    ] {
        let bytes = postcard::to_allocvec(&value).unwrap();
        assert_eq!(
            postcard::from_bytes::<ConfigurationValue>(&bytes).unwrap(),
            value
        );
        assert_eq!(
            postcard::from_bytes::<Legacy>(&bytes).unwrap().current(),
            value
        );
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<Legacy>(&json).unwrap().current(),
            value
        );
        assert_eq!(
            serde_json::from_str::<ConfigurationValue>(&json).unwrap(),
            value
        );
        match value {
            ConfigurationValue::U64(v) if v > MAXIMUM => {
                assert_eq!(json, format!("{{\"U64\":\"{v}\"}}"))
            }
            ConfigurationValue::I64(v) if !(-(MAXIMUM as i64)..=MAXIMUM as i64).contains(&v) => {
                assert_eq!(json, format!("{{\"I64\":\"{v}\"}}"))
            }
            ConfigurationValue::U64(v) => assert_eq!(json, format!("{{\"U64\":{v}}}")),
            ConfigurationValue::I64(v) => assert_eq!(json, format!("{{\"I64\":{v}}}")),
            _ => unreachable!(),
        }
    }
}
