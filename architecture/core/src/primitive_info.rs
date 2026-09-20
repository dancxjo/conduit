//! Canonical encodings for the foundational portable Info leaves.
//!
//! This is a small reviewed registry, not a dynamic runtime type system.
//! Domain-owned leaves remain the responsibility of their semantic owners.

use crate::{InfoBool, InfoDecodeError, Quantity, QuantityDecodeRefusal, Scalar};

pub const UNIT_INFO_ID: &str = "value/unit";
pub const COUNT_INFO_ID: &str = "value/count";
pub const TEXT_INFO_ID: &str = "value/text";
pub const BYTES_INFO_ID: &str = "value/bytes";
pub const COUNT_ENCODED_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveInfoKind {
    Unit,
    Bool,
    Count,
    Scalar,
    Text,
    Bytes,
    Quantity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveInfoRefusal {
    UnitNotEmpty,
    Bool(InfoDecodeError),
    CountLength { actual: usize },
    Scalar(InfoDecodeError),
    TextUtf8,
    Quantity(QuantityDecodeRefusal),
}

pub const fn primitive_info_kind(identity: &str) -> Option<PrimitiveInfoKind> {
    match identity.as_bytes() {
        b"value/unit" => Some(PrimitiveInfoKind::Unit),
        b"value/bool" => Some(PrimitiveInfoKind::Bool),
        b"value/count" => Some(PrimitiveInfoKind::Count),
        b"value/scalar" => Some(PrimitiveInfoKind::Scalar),
        b"value/text" => Some(PrimitiveInfoKind::Text),
        b"value/bytes" => Some(PrimitiveInfoKind::Bytes),
        b"value/quantity" => Some(PrimitiveInfoKind::Quantity),
        _ => None,
    }
}

pub fn validate_primitive_info(identity: &str, encoded: &[u8]) -> Result<(), PrimitiveInfoRefusal> {
    match primitive_info_kind(identity) {
        Some(PrimitiveInfoKind::Unit) if !encoded.is_empty() => {
            Err(PrimitiveInfoRefusal::UnitNotEmpty)
        }
        Some(PrimitiveInfoKind::Unit | PrimitiveInfoKind::Bytes) | None => Ok(()),
        Some(PrimitiveInfoKind::Bool) => InfoBool::decode(encoded)
            .map(|_| ())
            .map_err(PrimitiveInfoRefusal::Bool),
        Some(PrimitiveInfoKind::Count) if encoded.len() != COUNT_ENCODED_LEN => {
            Err(PrimitiveInfoRefusal::CountLength {
                actual: encoded.len(),
            })
        }
        Some(PrimitiveInfoKind::Count) => Ok(()),
        Some(PrimitiveInfoKind::Scalar) => Scalar::decode(encoded)
            .map(|_| ())
            .map_err(PrimitiveInfoRefusal::Scalar),
        Some(PrimitiveInfoKind::Text) => core::str::from_utf8(encoded)
            .map(|_| ())
            .map_err(|_| PrimitiveInfoRefusal::TextUtf8),
        Some(PrimitiveInfoKind::Quantity) => Quantity::decode(encoded)
            .map(|_| ())
            .map_err(PrimitiveInfoRefusal::Quantity),
    }
}

pub const fn encode_count(value: u64) -> [u8; COUNT_ENCODED_LEN] {
    value.to_le_bytes()
}

pub fn decode_count(encoded: &[u8]) -> Result<u64, PrimitiveInfoRefusal> {
    let bytes: [u8; COUNT_ENCODED_LEN] =
        encoded
            .try_into()
            .map_err(|_| PrimitiveInfoRefusal::CountLength {
                actual: encoded.len(),
            })?;
    Ok(u64::from_le_bytes(bytes))
}
