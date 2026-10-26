//! CDCI2 keeps complete typed requests and named-field presence. Only genuine
//! serde u8 sequences/tuples use a byte-string representation; public JSON and
//! all authoritative constructors/Plan/Show validation remain outside this codec.
mod access;
mod byte_probe;
mod decode;
mod encode;
mod positive;
#[cfg(test)]
mod tests;
mod visit;
use crate::wire::{MAX_BLOB_BYTES, Refusal};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
const PREFIX: &[u8] = b"CDCI\x02";
const DEPTH: u8 = 64;
pub(crate) fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, Refusal> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_BLOB_BYTES as usize)
        .map_err(|_| Refusal::Capacity)?;
    bytes.resize(MAX_BLOB_BYTES as usize, 0);
    bytes[..PREFIX.len()].copy_from_slice(PREFIX);
    let mut remaining = &mut bytes[PREFIX.len()..];
    ciborium::ser::into_writer(&PackedValue(value), &mut remaining)
        .map_err(|_| Refusal::Capacity)?;
    let length = MAX_BLOB_BYTES as usize - remaining.len();
    bytes.truncate(length);
    Ok(bytes)
}
struct PackedValue<T>(T);
impl<T: Serialize> Serialize for PackedValue<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(encode::Packed(serializer))
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for PackedValue<T> {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        T::deserialize(decode::Sequences {
            inner: decoder,
            depth: DEPTH,
        })
        .map(Self)
    }
}
pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Refusal> {
    if bytes.is_empty() || bytes.len() > MAX_BLOB_BYTES as usize {
        return Err(Refusal::Bounds);
    }
    let mut remaining = bytes.strip_prefix(PREFIX).ok_or(Refusal::Malformed)?;
    let PackedValue(value) =
        ciborium::de::from_reader_with_recursion_limit(&mut remaining, usize::from(DEPTH))
            .map_err(|_| Refusal::Malformed)?;
    if !remaining.is_empty() {
        return Err(Refusal::Malformed);
    }
    Ok(value)
}
