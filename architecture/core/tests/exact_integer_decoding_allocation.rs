//! Preparation allocations only; input is built before each observation.
use conduit_core::ConfigurationValue;
use serde::{Deserialize, Deserializer};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;

#[derive(Deserialize)]
enum Legacy {
    U64(#[serde(deserialize_with = "legacy_unsigned")] u64),
    I64(#[serde(deserialize_with = "legacy_signed")] i64),
}
fn legacy_unsigned<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    use serde::de::Error as _;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum HumanInteger {
        Number(u64),
        Decimal(String),
    }
    match HumanInteger::deserialize(deserializer)? {
        HumanInteger::Number(value) if value <= 9_007_199_254_740_991 => Ok(value),
        HumanInteger::Number(value) => Err(D::Error::custom(format!("JSON integer {value} exceeds JavaScript's exact integer range; encode it as a decimal string"))),
        HumanInteger::Decimal(value) => value.parse().map_err(|_| D::Error::custom("invalid exact u64 decimal string")),
    }
}
fn legacy_signed<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    use serde::de::Error as _;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum HumanInteger {
        Number(i64),
        Decimal(String),
    }
    match HumanInteger::deserialize(deserializer)? {
        HumanInteger::Number(value) if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&value) => Ok(value),
        HumanInteger::Number(value) => Err(D::Error::custom(format!("JSON integer {value} exceeds JavaScript's exact integer range; encode it as a decimal string"))),
        HumanInteger::Decimal(value) => value.parse().map_err(|_| D::Error::custom("invalid exact i64 decimal string")),
    }
}
fn value(value: Legacy) -> ConfigurationValue {
    match value {
        Legacy::U64(v) => ConfigurationValue::U64(v),
        Legacy::I64(v) => ConfigurationValue::I64(v),
    }
}
fn observations(text: &str) -> (probe::Observation, probe::Observation) {
    let (old, old_cost) = probe::observe(|| serde_json::from_str::<Legacy>(text).map(value));
    let (new, new_cost) = probe::observe(|| serde_json::from_str::<ConfigurationValue>(text));
    assert_eq!(old.as_ref().ok(), new.as_ref().ok());
    assert_eq!(old.is_ok(), new.is_ok());
    (old_cost, new_cost)
}
#[test]
fn ordinary_exact_integer_scalars_remain_allocation_free() {
    for text in [
        "{\"U64\":42}",
        "{\"I64\":-42}",
        "{\"U64\":\"00042\"}",
        "{\"I64\":\"-00042\"}",
    ] {
        let (_, new) = observations(text);
        assert_eq!(new.allocations, 0, "{text}: {new:?}");
        assert_eq!(new.peak_bytes, 0, "{text}: {new:?}");
    }
}
#[test]
fn large_unescaped_zero_decimal_does_not_copy_content_into_owned_string() {
    let length = 64 * 1024;
    for variant in ["U64", "I64"] {
        let text = format!("{{\"{variant}\":\"{}\"}}", "0".repeat(length));
        let (old, new) = observations(&text);
        assert!(old.peak_bytes >= length, "{old:?}");
        assert_eq!(new.allocations, 0, "{new:?}");
        assert_eq!(new.peak_bytes, 0, "{new:?}");
    }
}
#[test]
fn escaped_zero_decimal_keeps_parser_scratch_but_removes_content_copies() {
    let length = 64 * 1024;
    for variant in ["U64", "I64"] {
        let text = format!("{{\"{variant}\":\"{}\"}}", "\\u0030".repeat(length));
        let (old, new) = observations(&text);
        assert!(
            new.peak_bytes >= length,
            "parser scratch was not observed: {new:?}"
        );
        assert!(
            old.peak_bytes >= new.peak_bytes + length,
            "old {old:?}; new {new:?}"
        );
        assert!(
            old.requested_bytes > new.requested_bytes,
            "old {old:?}; new {new:?}"
        );
    }
}
#[test]
fn invalid_nested_integer_container_is_rejected_before_content_buffering() {
    let items = 8192;
    let nested = format!("[{}]", "[0],".repeat(items).trim_end_matches(','));
    for variant in ["U64", "I64"] {
        let text = format!("{{\"{variant}\":{nested}}}");
        let (old, new) = observations(&text);
        assert!(serde_json::from_str::<ConfigurationValue>(&text).is_err());
        assert!(old.peak_bytes > items, "{old:?}");
        // Error text/position may allocate; only the nested Content tree must
        // disappear. This is not a claim that refused input costs zero bytes.
        assert!(new.peak_bytes < 1024, "{new:?}");
    }
}
