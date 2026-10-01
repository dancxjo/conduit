//! Allocation-free nominal primitive framing for the prepared evaluator.

use super::{PrimitiveValue, Refusal};
use alloc::vec::Vec;

pub(super) fn input_payload<'a>(
    encoded: &'a [u8],
    expected_type: &[u8],
) -> Result<&'a [u8], Refusal> {
    leaf_payload(
        encoded
            .strip_prefix(expected_type)
            .ok_or(Refusal::InvalidInput)?,
    )
}

pub(super) fn append_output(out: &mut Vec<u8>, value_type: &[u8], value: &PrimitiveValue) {
    out.extend_from_slice(value_type);
    out.push(0);
    out.extend_from_slice(&(value.length as u32).to_le_bytes());
    out.extend_from_slice(value.as_slice());
}

fn leaf_payload(encoded: &[u8]) -> Result<&[u8], Refusal> {
    let [0, length @ ..] = encoded else {
        return Err(Refusal::InvalidInput);
    };
    if length.len() < 4 {
        return Err(Refusal::InvalidInput);
    }
    let byte_len = u32::from_le_bytes(length[..4].try_into().unwrap()) as usize;
    let bytes = &length[4..];
    (bytes.len() == byte_len)
        .then_some(bytes)
        .ok_or(Refusal::InvalidInput)
}
