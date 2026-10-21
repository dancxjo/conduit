//! Private application exchange JSON. This is never graph custody encoding.
//!
//! Every array is `[length, elements...]`, including tuples and tuple variants.
//! A no-allocation syntax/count preflight authenticates every hint before typed
//! deserialization. Version and framing overhead share the existing decoded
//! byte ceiling; this does not enlarge wire, heap or execution grants.
mod backend;
mod de;
mod preflight;
mod ser;
use super::Refusal;
use serde::{Serialize, de::DeserializeOwned};

const VERSION: &str = "CAPSEQ1";
const PREFIX: &[u8] = b"[2,\"CAPSEQ1\",";
const MAX_BYTES: usize = super::MAX_DECODED_JSON_BYTES;

/// Serialize directly into the admitted reusable JSON workspace.
pub(crate) fn encode_to_slice<T: Serialize + ?Sized>(
    value: &T,
    output: &mut [u8],
) -> Result<usize, Refusal> {
    let length = backend::encode(&ser::Envelope(value), output)?;
    preflight::validate(&output[..length])?;
    Ok(length)
}

/// No typed Deserialize visitor runs until every sequence hint is verified.
pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Refusal> {
    if !bytes.starts_with(PREFIX) {
        return Err(Refusal::Malformed);
    }
    preflight::validate(bytes)?;
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = serde::de::DeserializeSeed::deserialize(de::EnvelopeSeed::<T>::new(), &mut decoder)
        .map_err(|_| Refusal::Malformed)?;
    decoder.end().map_err(|_| Refusal::Malformed)?;
    Ok(value)
}

#[cfg(test)]
mod tests;
