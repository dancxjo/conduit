//! Versioned finite IPC values, never semantic identities or effect authority.
//! Decode fully, then admit the external capability, exact service instance,
//! request ordering and work/output grant before invoking any core operation.
mod common;
mod request;
mod response;
pub use request::{Request, decode_request, encode_request};
mod pixels;
use crate::{Error, FramePlan, FrameRect, Handle, MAX_CHUNK, MAX_PIXELS, MAX_SURFACES, Rect};
use common::*;
pub use pixels::{PixelBytes, decode_pixels, encode_pixels};
pub use response::{FrameSpan, Response, decode_response, encode_response};

pub const EXCHANGE_BYTES: usize = 4096;
pub const OUTPUT_BYTES: u32 = 65536;
pub const HEADER_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    Truncated,
    Length,
    Version,
    Reserved,
    Operation,
    Value,
}
/// Boot-local correlation only. A decoded instance is a claim, not admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub instance: u64,
    pub request: u64,
}
fn valid_handle(h: Handle) -> Result<(), CodecError> {
    if usize::from(h.slot) >= MAX_SURFACES || h.generation == 0 {
        Err(CodecError::Value)
    } else {
        Ok(())
    }
}
fn valid_rect(r: Rect) -> Result<(), CodecError> {
    r.pixels().map(|_| ()).map_err(|_| CodecError::Value)
}
fn nonzero(v: u64) -> Result<(), CodecError> {
    if v == 0 {
        Err(CodecError::Value)
    } else {
        Ok(())
    }
}
fn limit(v: u32) -> Result<(), CodecError> {
    if v == 0 || v > MAX_CHUNK as u32 {
        Err(CodecError::Value)
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests;

fn window_range(offset: u32, pixels: u32) -> Result<(), CodecError> {
    limit(pixels)?;
    if !offset.is_multiple_of(4)
        || pixels
            .checked_mul(4)
            .and_then(|bytes| offset.checked_add(bytes))
            .is_none_or(|end| end > OUTPUT_BYTES)
    {
        return Err(CodecError::Value);
    }
    Ok(())
}
