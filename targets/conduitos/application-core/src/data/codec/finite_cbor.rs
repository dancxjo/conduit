//! Shared finite private carrier: bounded slice serialization, allocation-free
//! preflight, then ordinary typed deserialization of retained immutable bytes.
mod decoder;
mod packing;
mod preflight;
use super::Refusal;
use serde::{Serialize, de::DeserializeOwned};

pub(super) fn encode<T: Serialize>(value: &T, output: &mut [u8]) -> Result<usize, Refusal> {
    let capacity = output.len();
    let mut remaining = &mut *output;
    ciborium::ser::into_writer(value, &mut remaining).map_err(|_| Refusal::Capacity)?;
    let length = capacity - remaining.len();
    packing::compact(&mut output[..length])
}
pub(super) fn validate(bytes: &[u8], maximum_plan: usize) -> Result<(), Refusal> {
    preflight::validate(bytes, maximum_plan)
}
/// Only immutable bytes already checked by Workspace can enter typed decoding.
pub(super) fn decode_preflighted<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Refusal> {
    decoder::decode(bytes).map_err(|_| Refusal::Malformed)
}
