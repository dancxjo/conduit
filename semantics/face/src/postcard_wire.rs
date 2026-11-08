//! Fixed-buffer representation of complete Face and Show objects.
//!
//! This ABI uses Postcard 1's Serde encoding of the existing public objects.
//! It carries no admission or execution authority. Callers retain responsibility
//! for validation, provenance, finite buffers, and the selected wire contract.
//! Encoding does not validate or allocate. Owned decoding belongs in separately
//! funded callers; they must reject trailing bytes and validate decoded objects.

use crate::{Face, Show};

/// Change this identity when the encoded representation changes incompatibly.
pub const ABI: &str = "conduit.face/postcard@1";

pub fn encode_face<'a>(face: &Face, output: &'a mut [u8]) -> Result<&'a mut [u8], postcard::Error> {
    postcard::to_slice(face, output)
}

pub fn encode_show<'a>(show: &Show, output: &'a mut [u8]) -> Result<&'a mut [u8], postcard::Error> {
    postcard::to_slice(show, output)
}
