//! Bound decoder allocations before invoking owned protocol deserialization.
use serde::de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};

const MAX_DEPTH: u8 = 16;
const ID_BYTES: usize = 256;
const DIAGNOSTIC_BYTES: usize = 512;

pub(crate) fn reply(bytes: &[u8]) -> Result<(), String> {
    // A JSON escape can encode one byte in six input bytes. Reject giant
    // strings before a decoder can format them into an enum/field error.
    let mut quoted = false;
    let mut escaped = false;
    let mut length = 0;
    for &byte in bytes {
        if quoted {
            if byte == b'"' && !escaped {
                quoted = false;
            } else {
                length += 1;
                if length > DIAGNOSTIC_BYTES * 6 {
                    return Err("startup reply string bound".into());
                }
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                }
            }
        } else if byte == b'"' {
            quoted = true;
            length = 0;
        }
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    Shape {
        depth: 0,
        diagnostic: false,
        signature: false,
    }
    .deserialize(&mut decoder)
    .map_err(|_| "startup reply shape bound")?;
    decoder
        .end()
        .map_err(|_| "startup reply trailing input".into())
}

struct Shape {
    depth: u8,
    diagnostic: bool,
    signature: bool,
}
impl<'de> DeserializeSeed<'de> for Shape {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, decoder: D) -> Result<(), D::Error> {
        if self.depth > MAX_DEPTH {
            return Err(D::Error::custom("reply depth"));
        }
        decoder.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Shape {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("bounded startup reply")
    }
    fn visit_bool<E: Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: Error>(self, value: &str) -> Result<(), E> {
        if value.len()
            > if self.diagnostic {
                DIAGNOSTIC_BYTES
            } else {
                ID_BYTES
            }
        {
            return Err(E::custom("reply string"));
        }
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        let limit = if self.signature {
            64
        } else {
            conduit_std_host::body_execution::STARTUP_MEMBER_MAXIMUM_CHIME_RECEIPTS
        };
        let mut count = 0;
        while sequence
            .next_element_seed(Shape {
                depth: self.depth + 1,
                diagnostic: false,
                signature: false,
            })?
            .is_some()
        {
            count += 1;
            if count > limit {
                return Err(A::Error::custom("reply sequence"));
            }
        }
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut count = 0;
        while let Some(field) = map.next_key_seed(Field)? {
            count += 1;
            if count > 32 {
                return Err(A::Error::custom("reply fields"));
            }
            map.next_value_seed(Shape {
                depth: self.depth + 1,
                diagnostic: field.0,
                signature: field.1,
            })?;
        }
        Ok(())
    }
}
struct Field;
impl<'de> DeserializeSeed<'de> for Field {
    type Value = (bool, bool);
    fn deserialize<D: serde::Deserializer<'de>>(self, decoder: D) -> Result<Self::Value, D::Error> {
        decoder.deserialize_identifier(self)
    }
}
impl Visitor<'_> for Field {
    type Value = (bool, bool);
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("bounded reply field")
    }
    fn visit_str<E: Error>(self, key: &str) -> Result<Self::Value, E> {
        if key.len() > ID_BYTES {
            return Err(E::custom("reply key"));
        }
        Ok((
            matches!(key, "failure" | "cleanup_failure"),
            matches!(key, "member_signature" | "owner_signature"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reply_preflight_bounds_unknown_errors_before_owned_decoding() {
        for text in ["x".repeat(512 * 1024), "\\u0078".repeat(1024)] {
            let bytes = format!("{{\"reply\":{{\"outcome\":\"{text}\"}}}}");
            assert!(reply(bytes.as_bytes()).is_err());
        }
        assert!(reply(format!("{{\"{}\":0}}", "k".repeat(257)).as_bytes()).is_err());
        assert!(reply(format!("{{\"kind\":\"{}\"}}", "x".repeat(257)).as_bytes()).is_err());
        assert!(reply(format!("{{\"failure\":\"{}\"}}", "x".repeat(512)).as_bytes()).is_ok());
        assert!(reply(format!("{{\"failure\":\"{}\"}}", "x".repeat(513)).as_bytes()).is_err());
    }
    #[test]
    fn reply_preflight_retains_fixed_signature_and_kernel_receipt_limits() {
        let array = |n| format!("[{}]", vec!["0"; n].join(","));
        assert!(reply(format!("{{\"member_signature\":{}}}", array(64)).as_bytes()).is_ok());
        assert!(reply(format!("{{\"member_signature\":{}}}", array(65)).as_bytes()).is_err());
        let bound = conduit_std_host::body_execution::STARTUP_MEMBER_MAXIMUM_CHIME_RECEIPTS;
        assert!(reply(format!("{{\"startup_chimes\":{}}}", array(bound)).as_bytes()).is_ok());
        assert!(reply(format!("{{\"startup_chimes\":{}}}", array(bound + 1)).as_bytes()).is_err());
        assert!(reply(b"{}{}").is_err());
    }
}
