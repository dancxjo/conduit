//! Human exact integers without buffering an untagged Content tree.
//! Parser-owned escape scratch remains the carrier's preparation responsibility.
use super::MAXIMUM_EXACT_JAVASCRIPT_INTEGER;
use alloc::format;
use core::fmt;
use serde::{
    de::{Error, Visitor},
    Deserializer,
};

const UNTAGGED_REFUSAL: &str = "data did not match any variant of untagged enum HumanInteger";

fn range_error<E: Error>(value: impl fmt::Display) -> E {
    E::custom(format!("JSON integer {value} exceeds JavaScript's exact integer range; encode it as a decimal string"))
}

pub(super) fn deserialize_unsigned<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<u64, D::Error> {
    struct Unsigned;
    impl Visitor<'_> for Unsigned {
        type Value = u64;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an exact unsigned integer or decimal string")
        }
        fn visit_u64<E: Error>(self, value: u64) -> Result<u64, E> {
            if value <= MAXIMUM_EXACT_JAVASCRIPT_INTEGER {
                Ok(value)
            } else {
                Err(range_error(value))
            }
        }
        fn visit_i64<E: Error>(self, value: i64) -> Result<u64, E> {
            let value = u64::try_from(value).map_err(|_| E::custom(UNTAGGED_REFUSAL))?;
            self.visit_u64(value)
        }
        fn visit_str<E: Error>(self, value: &str) -> Result<u64, E> {
            value
                .parse()
                .map_err(|_| E::custom("invalid exact u64 decimal string"))
        }
        fn visit_bytes<E: Error>(self, value: &[u8]) -> Result<u64, E> {
            let value = core::str::from_utf8(value).map_err(|_| E::custom(UNTAGGED_REFUSAL))?;
            self.visit_str(value)
        }
        // The default would coerce char to str. The old untagged String arm
        // rejects Content::Char, even if it happens to contain a decimal digit.
        fn visit_char<E: Error>(self, _: char) -> Result<u64, E> {
            Err(E::custom(UNTAGGED_REFUSAL))
        }
    }
    deserializer.deserialize_any(Unsigned)
}

pub(super) fn deserialize_signed<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<i64, D::Error> {
    struct Signed;
    impl Visitor<'_> for Signed {
        type Value = i64;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an exact signed integer or decimal string")
        }
        fn visit_i64<E: Error>(self, value: i64) -> Result<i64, E> {
            let maximum = MAXIMUM_EXACT_JAVASCRIPT_INTEGER as i64;
            if (-maximum..=maximum).contains(&value) {
                Ok(value)
            } else {
                Err(range_error(value))
            }
        }
        fn visit_u64<E: Error>(self, value: u64) -> Result<i64, E> {
            let value = i64::try_from(value).map_err(|_| E::custom(UNTAGGED_REFUSAL))?;
            self.visit_i64(value)
        }
        fn visit_str<E: Error>(self, value: &str) -> Result<i64, E> {
            value
                .parse()
                .map_err(|_| E::custom("invalid exact i64 decimal string"))
        }
        fn visit_bytes<E: Error>(self, value: &[u8]) -> Result<i64, E> {
            let value = core::str::from_utf8(value).map_err(|_| E::custom(UNTAGGED_REFUSAL))?;
            self.visit_str(value)
        }
        fn visit_char<E: Error>(self, _: char) -> Result<i64, E> {
            Err(E::custom(UNTAGGED_REFUSAL))
        }
    }
    deserializer.deserialize_any(Signed)
}
