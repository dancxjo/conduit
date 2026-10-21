//! Finite transport encoding; compression never changes typed application data.
mod byte_sequences;
use super::Refusal;
use crate::MAX_BLOB;
use alloc::vec::Vec;
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

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Refusal> {
    if bytes.is_empty() || bytes.len() > MAX_BLOB {
        return Err(Refusal::Bounds);
    }
    if bytes.starts_with(MAGIC) {
        let length = bytes.get(MAGIC.len()..HEADER).ok_or(Refusal::Malformed)?;
        let length =
            u32::from_le_bytes(length.try_into().map_err(|_| Refusal::Malformed)?) as usize;
        if length == 0 || length > MAX_DECODED_JSON_BYTES {
            return Err(Refusal::Bounds);
        }
        let mut json = buffer(length)?;
        let compressed = &bytes[HEADER..];
        let (status, consumed, written) = decompress(
            &mut DecompressorOxide::new(),
            compressed,
            &mut json,
            0,
            inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
                | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
        );
        if status != TINFLStatus::Done || consumed != compressed.len() || written != length {
            return Err(Refusal::Malformed);
        }
        serde_json::from_slice(&json).map_err(|_| Refusal::Malformed)
    } else {
        serde_json::from_slice(bytes).map_err(|_| Refusal::Malformed)
    }
}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, Refusal> {
    // Preserve the original raw bytes for small values, including digest inputs.
    let mut json = buffer(MAX_BLOB)?;
    if let Ok(length) = serde_json_core::to_slice(&JsonBytes(value), &mut json) {
        json.truncate(length);
        return Ok(json);
    }
    drop(json);
    let mut json = buffer(MAX_DECODED_JSON_BYTES)?;
    let length =
        serde_json_core::to_slice(&JsonBytes(value), &mut json).map_err(|_| Refusal::Capacity)?;
    json.truncate(length);
    let mut output = buffer(MAX_BLOB)?;
    output[..MAGIC.len()].copy_from_slice(MAGIC);
    output[MAGIC.len()..HEADER].copy_from_slice(&(length as u32).to_le_bytes());
    let mut compressor = CompressorOxide::new(create_comp_flags_from_zip_params(6, 15, 0));
    let (status, consumed, written) = compress(
        &mut compressor,
        &json,
        &mut output[HEADER..],
        TDEFLFlush::Finish,
    );
    if status != TDEFLStatus::Done || consumed != json.len() {
        return Err(Refusal::Capacity);
    }
    output.truncate(HEADER + written);
    Ok(output)
}

#[cfg(test)]
mod tests;
