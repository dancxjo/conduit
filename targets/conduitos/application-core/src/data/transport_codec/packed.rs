//! Canonical packed bytes for the private V2 canonical_value field only.
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
/// The compact branch lives at the reserved field boundary, never in the
/// general sequence decoder. Structs/enums/widened numbers cannot enter it.
pub(super) struct ValueSeed<S>(pub(super) S);
impl<'de, S: DeserializeSeed<'de>> DeserializeSeed<'de> for ValueSeed<S> {
    type Value = S::Value;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.0.deserialize(read(d)?)
    }
}
/// This parse depends on the wire reader, not the destination seed. Sharing it
/// prevents one copy of JSON string parsing for every containing field Type.
#[inline(never)]
fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Only<'de, D::Error>, D::Error> {
    let value = <&'de str>::deserialize(d)?;
    let length = extent(value).map_err(|_| D::Error::custom("invalid packed bytes"))?;
    Ok(Only {
        value,
        length,
        error: core::marker::PhantomData,
    })
}

struct Only<'a, E> {
    value: &'a str,
    length: usize,
    error: core::marker::PhantomData<E>,
}
impl<'de, E: Error> Deserializer<'de> for Only<'de, E> {
    type Error = E;
    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
        let mut sequence = Bytes {
            encoded: &self.value.as_bytes()[PREFIX.len()..],
            length: self.length,
            index: 0,
            triplet: [0; 3],
            triplet_index: 3,
            error: core::marker::PhantomData,
        };
        let result = visitor.visit_seq(&mut sequence)?;
        if sequence.index != self.length {
            return Err(E::custom("packed extent differs"));
        }
        Ok(result)
    }
    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("packed field requires u8 sequence"))
    }
    serde::forward_to_deserialize_any! {bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option unit unit_struct newtype_struct tuple tuple_struct map struct enum identifier ignored_any}
}
struct Bytes<'a, E> {
    encoded: &'a [u8],
    length: usize,
    index: usize,
    triplet: [u8; 3],
    triplet_index: usize,
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
        if self.triplet_index == 3 {
            // extent() already validated the entire canonical input, including
            // padding. Decode each quartet once; retain only its three bytes,
            // not another allocation or a second decoded document.
            let offset = self.index / 3 * 4;
            let remaining = self.length - self.index;
            let a = sextet(self.encoded[offset]).map_err(|_| E::custom("invalid packed byte"))?;
            let b =
                sextet(self.encoded[offset + 1]).map_err(|_| E::custom("invalid packed byte"))?;
            self.triplet[0] = (a << 2) | (b >> 4);
            if remaining > 1 {
                let c = sextet(self.encoded[offset + 2])
                    .map_err(|_| E::custom("invalid packed byte"))?;
                self.triplet[1] = (b << 4) | (c >> 2);
                if remaining > 2 {
                    let d = sextet(self.encoded[offset + 3])
                        .map_err(|_| E::custom("invalid packed byte"))?;
                    self.triplet[2] = (c << 6) | d;
                }
            }
            self.triplet_index = 0;
        }
        let byte = self.triplet[self.triplet_index];
        self.triplet_index += 1;
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
