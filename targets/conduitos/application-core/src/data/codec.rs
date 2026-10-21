//! Finite transport encoding; compression never changes typed application data.
mod byte_sequences;
use super::{Command, Refusal, Reply, transport_codec};
use crate::MAX_BLOB;
use alloc::{boxed::Box, vec::Vec};
use byte_sequences::JsonBytes;
use miniz_oxide::{
    deflate::core::{
        CompressorOxide, TDEFLFlush, TDEFLStatus, compress, create_comp_flags_from_zip_params,
    },
    inflate::{
        TINFLStatus,
        core::{DecompressorOxide, decompress, inflate_flags},
    },
};
use serde::{Serialize, de::DeserializeOwned};

/// A decoded wire engineering ceiling, not proof of aggregate service fit.
/// The wire remains at MAX_BLOB; neither ceiling authorizes an application effect.
pub const MAX_DECODED_JSON_BYTES: usize = 1024 * 1024;
const MAGIC: &[u8; 8] = b"CAPZ\x01\0\0\0";
const HEADER: usize = MAGIC.len() + 4;

fn buffer(bytes: usize) -> Result<Vec<u8>, Refusal> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(bytes)
        .map_err(|_| Refusal::Capacity)?;
    output.resize(bytes, 0);
    Ok(output)
}

fn decoded_length(bytes: &[u8]) -> Result<Option<usize>, Refusal> {
    if bytes.is_empty() || bytes.len() > MAX_BLOB {
        return Err(Refusal::Bounds);
    }
    if !bytes.starts_with(MAGIC) {
        return Ok(None);
    }
    let length = bytes.get(MAGIC.len()..HEADER).ok_or(Refusal::Malformed)?;
    let length = u32::from_le_bytes(length.try_into().map_err(|_| Refusal::Malformed)?) as usize;
    if length == 0 || length > MAX_DECODED_JSON_BYTES {
        return Err(Refusal::Bounds);
    }
    Ok(Some(length))
}
fn decoded_json<'a>(bytes: &'a [u8], json: &'a mut [u8]) -> Result<&'a [u8], Refusal> {
    let Some(length) = decoded_length(bytes)? else {
        return Ok(bytes);
    };
    let json = json.get_mut(..length).ok_or(Refusal::Capacity)?;
    let compressed = &bytes[HEADER..];
    let (status, consumed, written) = decompress(
        &mut DecompressorOxide::new(),
        compressed,
        json,
        0,
        inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
            | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
    );
    if status != TINFLStatus::Done || consumed != compressed.len() || written != length {
        return Err(Refusal::Malformed);
    }
    Ok(json)
}
fn decode_in<T: DeserializeOwned>(bytes: &[u8], json: &mut [u8]) -> Result<T, Refusal> {
    serde_json::from_slice(decoded_json(bytes, json)?).map_err(|_| Refusal::Malformed)
}
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Refusal> {
    match decoded_length(bytes)? {
        Some(length) => decode_in(bytes, &mut buffer(length)?),
        None => decode_in(bytes, &mut []),
    }
}
/// Private service replies use exact sequence framing, never graph custody JSON.
pub fn decode_reply(bytes: &[u8]) -> Result<Reply, Refusal> {
    match decoded_length(bytes)? {
        Some(length) => transport_codec::decode(decoded_json(bytes, &mut buffer(length)?)?),
        None => transport_codec::decode(decoded_json(bytes, &mut [])?),
    }
}
/// One admitted reusable JSON workspace, reserved before retained controllers.
/// Graph methods preserve canonical JSON/custody bytes. Private service methods
/// use versioned exact-sequence framing in the same admitted storage.
pub struct Workspace {
    json: Vec<u8>,
    compressor: Box<CompressorOxide>,
}
impl Workspace {
    pub fn new() -> Result<Self, Refusal> {
        Ok(Self {
            json: buffer(MAX_DECODED_JSON_BYTES)?,
            compressor: Box::new(CompressorOxide::new(create_comp_flags_from_zip_params(
                6, 15, 0,
            ))),
        })
    }
    pub fn decode<T: DeserializeOwned>(&mut self, bytes: &[u8]) -> Result<T, Refusal> {
        decode_in(bytes, &mut self.json)
    }
    pub fn decode_command(&mut self, bytes: &[u8]) -> Result<Command, Refusal> {
        transport_codec::decode(decoded_json(bytes, &mut self.json)?)
    }
    /// For large target commands, do decompression and the complete framing
    /// preflight in their own protected entry. State freezes incoming bytes
    /// until the following Commit, so verified allocation hints stay trusted.
    pub(crate) fn preflight_command(&mut self, bytes: &[u8]) -> Result<usize, Refusal> {
        let length = {
            let decoded = decoded_json(bytes, &mut self.json)?;
            transport_codec::preflight(decoded)?;
            decoded.len()
        };
        if !bytes.starts_with(MAGIC) {
            self.json[..length].copy_from_slice(bytes);
        }
        Ok(length)
    }
    pub(crate) fn decode_preflighted_command(&self, length: usize) -> Result<Command, Refusal> {
        transport_codec::decode_validated(self.json.get(..length).ok_or(Refusal::Bounds)?)
    }
    /// While a full graph is retained, reject every other command tag before
    /// decoding any second payload into the same fixed grant.
    pub(crate) fn decode_projection_finish(&mut self, bytes: &[u8]) -> Result<Command, Refusal> {
        #[derive(serde::Deserialize)]
        enum FinishOnly {
            FinishGraphProjection,
        }
        let _: FinishOnly = transport_codec::decode(decoded_json(bytes, &mut self.json)?)
            .map_err(|_| Refusal::Order)?;
        Ok(Command::FinishGraphProjection)
    }
    pub(crate) fn encode_reply_into(
        &mut self,
        reply: &Reply,
        output: &mut Vec<u8>,
    ) -> Result<(), Refusal> {
        if output.capacity() < MAX_BLOB {
            return Err(Refusal::Capacity);
        }
        output.resize(MAX_BLOB, 0);
        let length = transport_codec::encode_to_slice(reply, &mut self.json)?;
        self.finish_encoded(length, output)
    }
    pub fn encode<T: Serialize>(&mut self, value: &T) -> Result<Vec<u8>, Refusal> {
        let mut output = buffer(MAX_BLOB)?;
        self.encode_into(value, &mut output)?;
        Ok(output)
    }
    pub(crate) fn encode_into<T: Serialize>(
        &mut self,
        value: &T,
        output: &mut Vec<u8>,
    ) -> Result<(), Refusal> {
        if output.capacity() < MAX_BLOB {
            return Err(Refusal::Capacity);
        }
        output.resize(MAX_BLOB, 0);
        let length = serde_json_core::to_slice(&JsonBytes(value), &mut self.json)
            .map_err(|_| Refusal::Capacity)?;
        self.finish_encoded(length, output)
    }
    fn finish_encoded(&mut self, length: usize, output: &mut Vec<u8>) -> Result<(), Refusal> {
        let json = &self.json[..length];
        if length > MAX_BLOB {
            self.compressor.reset();
            return compress_json_into(json, &mut self.compressor, output);
        }
        output[..length].copy_from_slice(json);
        output.truncate(length);
        Ok(())
    }
}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, Refusal> {
    encode_with(value, |value, output| {
        serde_json_core::to_slice(&JsonBytes(value), output).map_err(|_| Refusal::Capacity)
    })
}
/// Encode one private Application command with exact sequence framing.
pub fn encode_command(command: &Command) -> Result<Vec<u8>, Refusal> {
    encode_with(command, transport_codec::encode_to_slice)
}
fn encode_with<T: Serialize>(
    value: &T,
    serialize: impl Fn(&T, &mut [u8]) -> Result<usize, Refusal>,
) -> Result<Vec<u8>, Refusal> {
    // Finite serialization tiers avoid reserving a contiguous 1 MiB block for
    // a graph whose exact JSON fits 256 or 512 KiB. Failed serialization owns
    // no effects; each previous buffer is released before the next reservation.
    let mut encoded = None;
    for capacity in [MAX_BLOB, 2 * MAX_BLOB, 4 * MAX_BLOB, MAX_DECODED_JSON_BYTES] {
        let mut json = buffer(capacity)?;
        if let Ok(length) = serialize(value, &mut json) {
            json.truncate(length);
            encoded = Some(json);
            break;
        }
    }
    let json = encoded.ok_or(Refusal::Capacity)?;
    let length = json.len();
    if length <= MAX_BLOB {
        return Ok(json);
    }
    compress_json(&json)
}
fn compress_json(json: &[u8]) -> Result<Vec<u8>, Refusal> {
    let mut compressor = CompressorOxide::new(create_comp_flags_from_zip_params(6, 15, 0));
    compress_json_with(json, &mut compressor)
}
fn compress_json_with(json: &[u8], compressor: &mut CompressorOxide) -> Result<Vec<u8>, Refusal> {
    let mut output = buffer(MAX_BLOB)?;
    compress_json_into(json, compressor, &mut output)?;
    Ok(output)
}
fn compress_json_into(
    json: &[u8],
    compressor: &mut CompressorOxide,
    output: &mut Vec<u8>,
) -> Result<(), Refusal> {
    let length = json.len();
    output[..MAGIC.len()].copy_from_slice(MAGIC);
    output[MAGIC.len()..HEADER].copy_from_slice(&(length as u32).to_le_bytes());
    let (status, consumed, written) =
        compress(compressor, json, &mut output[HEADER..], TDEFLFlush::Finish);
    if status != TDEFLStatus::Done || consumed != json.len() {
        return Err(Refusal::Capacity);
    }
    output.truncate(HEADER + written);
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod transport_tests;
