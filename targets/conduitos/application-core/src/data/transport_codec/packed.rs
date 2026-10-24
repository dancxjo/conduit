//! Canonical packed bytes for the private V2 canonical_value field only.
use core::fmt;
use serde::de::*;
pub(super) const MAX_BYTES: usize = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;
pub(super) const MARKER: &str = "conduit.private/packed-u8@2";
pub(super) const PREFIX: &str = "~b64:";
pub(super) fn extent(value: &str) -> Result<usize, ()> {
    let bytes = value.strip_prefix(PREFIX).ok_or(())?.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(());
    }
    if bytes.is_empty() {
        return Ok(0);
    }
    let padding =
        usize::from(bytes[bytes.len() - 1] == b'=') + usize::from(bytes[bytes.len() - 2] == b'=');
    let length = (bytes.len() / 4)
        .checked_mul(3)
        .and_then(|n| n.checked_sub(padding))
        .ok_or(())?;
    if length > MAX_BYTES {
        return Err(());
    }
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'=' {
            if index < bytes.len() - padding {
                return Err(());
            }
        } else {
            sextet(byte)?;
        }
    }
    if (padding == 2 && sextet(bytes[bytes.len() - 3])? & 15 != 0)
        || (padding == 1 && sextet(bytes[bytes.len() - 2])? & 3 != 0)
    {
        return Err(());
    }
    Ok(length)
}
fn sextet(byte: u8) -> Result<u8, ()> {
    Ok(match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return Err(()),
    })
}
/// Typed preflight uses the exact same validator before any Serde string scratch.
/// Only called for canonical_value fields in V2; ordinary text has no reserved prefix.
pub(super) fn json_string(raw: &[u8]) -> Result<(), super::Refusal> {
    let raw = raw
        .strip_prefix(b"\"")
        .and_then(|v| v.strip_suffix(b"\""))
        .ok_or(super::Refusal::Malformed)?;
    let value = core::str::from_utf8(raw).map_err(|_| super::Refusal::Malformed)?;
    extent(value)
        .map(|_| ())
        .map_err(|_| super::Refusal::Malformed)
}
pub(super) struct SequenceVisitor<V> {
    pub(super) visitor: V,
    pub(super) compact: bool,
}
impl<'de, V: Visitor<'de>> Visitor<'de> for SequenceVisitor<V> {
    type Value = V::Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("counted array or canonical packed byte string")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
        super::de::visit_array(self.visitor, access, self.compact)
    }
    fn visit_borrowed_str<E: Error>(self, value: &'de str) -> Result<Self::Value, E> {
        let length = extent(value).map_err(|_| E::custom("invalid packed bytes"))?;
        let mut sequence = Bytes {
            encoded: &value.as_bytes()[PREFIX.len()..],
            length,
            index: 0,
            error: core::marker::PhantomData,
        };
        let result = self.visitor.visit_seq(&mut sequence)?;
        if sequence.index != length {
            return Err(E::custom("packed extent differs"));
        }
        Ok(result)
    }
    // Escaped/owned strings are not accepted. Preflight already rejects them.
}
struct Bytes<'a, E> {
    encoded: &'a [u8],
    length: usize,
    index: usize,
    error: core::marker::PhantomData<E>,
}
impl<'de, E: Error> SeqAccess<'de> for &mut Bytes<'_, E> {
    type Error = E;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, E> {
        if self.index == self.length {
            return Ok(None);
        }
        let offset = self.index / 3 * 4;
        let shift = self.index % 3;
        let a =
            sextet(self.encoded[offset + shift]).map_err(|_| E::custom("invalid packed byte"))?;
        let b = sextet(self.encoded[offset + shift + 1])
            .map_err(|_| E::custom("invalid packed byte"))?;
        let byte = match shift {
            0 => (a << 2) | (b >> 4),
            1 => (a << 4) | (b >> 2),
            _ => (a << 6) | b,
        };
        self.index += 1;
        seed.deserialize(Byte::<E>(byte, core::marker::PhantomData))
            .map(Some)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.length - self.index)
    }
}
struct Byte<E>(u8, core::marker::PhantomData<E>);
impl<'de, E: Error> Deserializer<'de> for Byte<E> {
    type Error = E;
    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
        visitor.visit_u8(self.0)
    }
    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("packed element requires u8"))
    }
    serde::forward_to_deserialize_any! {bool i8 i16 i32 i64 i128 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier ignored_any}
}
mod key;
pub(super) use key::KeySeed;

/// Match decoded ASCII field name without allocation, including escaped key spelling.
pub(super) fn is_key(raw: &[u8]) -> bool {
    let Some(mut raw) = raw.strip_prefix(b"\"").and_then(|v| v.strip_suffix(b"\"")) else {
        return false;
    };
    for expected in b"canonical_value" {
        let byte = if raw.starts_with(b"\\u") {
            let Some(hex) = raw.get(2..6) else {
                return false;
            };
            let mut n = 0u16;
            for &b in hex {
                let digit = match b {
                    b'0'..=b'9' => b - b'0',
                    b'a'..=b'f' => b - b'a' + 10,
                    b'A'..=b'F' => b - b'A' + 10,
                    _ => return false,
                };
                n = n * 16 + u16::from(digit);
            }
            raw = &raw[6..];
            if n > 127 {
                return false;
            }
            n as u8
        } else {
            let Some((&byte, rest)) = raw.split_first() else {
                return false;
            };
            raw = rest;
            byte
        };
        if byte != *expected {
            return false;
        }
    }
    raw.is_empty()
}

#[cfg(test)]
mod tests;
