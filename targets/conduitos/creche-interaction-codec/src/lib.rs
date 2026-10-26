//! Private immutable typed transfer; canonical interaction admission remains
//! with BirthDraft/Face. The named-map CBOR bytes and all finite bounds retain
//! CDCI version 1. This concrete codec has its own execution optimization scope.
#![no_std]
extern crate alloc;
use alloc::vec::Vec;
use conduit_birth_plot::BirthFaceBasis;
use conduit_face::{FaceInteraction, MaskShow};
use serde::{Deserialize, Serialize};

pub const MAX_BLOB_BYTES: u32 = 128 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Capacity,
    Malformed,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionRequest {
    pub revision: u32,
    pub basis: BirthFaceBasis,
    pub show: MaskShow,
    pub input: FaceInteraction,
}

const INTERACTION_ENCODING: &[u8] = b"CDCI\x01";
pub fn encode(value: &InteractionRequest) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_BLOB_BYTES as usize)
        .map_err(|_| Error::Capacity)?;
    bytes.resize(MAX_BLOB_BYTES as usize, 0);
    bytes[..INTERACTION_ENCODING.len()].copy_from_slice(INTERACTION_ENCODING);
    let mut remaining = &mut bytes[INTERACTION_ENCODING.len()..];
    ciborium::ser::into_writer(value, &mut remaining).map_err(|_| Error::Capacity)?;
    let length = MAX_BLOB_BYTES as usize - remaining.len();
    bytes.truncate(length);
    Ok(bytes)
}
pub fn decode(bytes: &[u8]) -> Result<InteractionRequest, Error> {
    if bytes.is_empty() || bytes.len() > MAX_BLOB_BYTES as usize {
        return Err(Error::Bounds);
    }
    let mut remaining = bytes
        .strip_prefix(INTERACTION_ENCODING)
        .ok_or(Error::Malformed)?;
    let value = ciborium::de::from_reader_with_recursion_limit(&mut remaining, 64)
        .map_err(|_| Error::Malformed)?;
    if !remaining.is_empty() {
        return Err(Error::Malformed);
    }
    Ok(value)
}
