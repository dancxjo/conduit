//! Canonical encodings for the foundational portable Info leaves.
//!
//! This is a small reviewed registry, not a dynamic runtime type system.
//! Domain-owned leaves remain the responsibility of their semantic owners.

use crate::{
    InfoBool, InfoDecodeError, Quantity, QuantityDimension, QuantityRefusal, Scalar, TerminalInfo,
    TerminalInfoDecodeRefusal,
};

pub const EMPTY_INFO_ID: &str = "value/empty";
pub const CANCELLATION_REQUEST_INFO_ID: &str = "control/cancellation-request";
pub const COUNT_INFO_ID: &str = "value/count";
pub const TEXT_INFO_ID: &str = "value/text";
pub const BYTES_INFO_ID: &str = "value/bytes";
pub const MAXIMUM_BYTES_INFO_BYTES: usize = 65_536;
pub const COUNT_ENCODED_LEN: usize = 8;
pub const F32_INFO_ID: &str = "value/ieee754-binary32";
pub const F64_INFO_ID: &str = "value/ieee754-binary64";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveInfoKind {
    Empty,
    CancellationRequest,
    Terminal,
    Bool,
    Count,
    Scalar,
    Text,
    Bytes,
    Quantity,
    Unit,
    Distance,
    Frequency,
    Duration,
    Voltage,
    Temperature,
    Angle,
    Ratio,
    PixelCount,
    U8,
    U16,
    U32,
    U64,
    U128,
    I8,
    I16,
    I32,
    I64,
    I128,
    F32,
    F64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveInfoRefusal {
    EmptyNotEmpty,
    WrongQuantityFamily,
    WrongQuantityRole,
    Bool(InfoDecodeError),
    CountLength {
        actual: usize,
    },
    Scalar(InfoDecodeError),
    TextUtf8,
    BytesTooLarge {
        maximum: usize,
        actual: usize,
    },
    Quantity(QuantityRefusal),
    Unit(crate::UnitRefusal),
    Terminal(TerminalInfoDecodeRefusal),
    WrongQuantityDimension {
        expected: QuantityDimension,
        actual: QuantityDimension,
    },
    IntegerLength {
        expected: usize,
        actual: usize,
    },
    FloatLength {
        expected: usize,
        actual: usize,
    },
}

pub fn primitive_info_kind(identity: &str) -> Option<PrimitiveInfoKind> {
    for (name, kind) in [
        (crate::DISTANCE_INFO_ID, PrimitiveInfoKind::Distance),
        (crate::FREQUENCY_INFO_ID, PrimitiveInfoKind::Frequency),
        (crate::DURATION_INFO_ID, PrimitiveInfoKind::Duration),
        (crate::VOLTAGE_INFO_ID, PrimitiveInfoKind::Voltage),
        (crate::TEMPERATURE_INFO_ID, PrimitiveInfoKind::Temperature),
        (crate::ANGLE_INFO_ID, PrimitiveInfoKind::Angle),
        (crate::RATIO_INFO_ID, PrimitiveInfoKind::Ratio),
        (crate::PIXEL_COUNT_INFO_ID, PrimitiveInfoKind::PixelCount),
    ] {
        if identity == name {
            return Some(kind);
        }
    }
    if crate::parse_quantity_role_info_id(identity).is_some() {
        return Some(PrimitiveInfoKind::Quantity);
    }
    match identity.as_bytes() {
        b"value/empty" => Some(PrimitiveInfoKind::Empty),
        b"control/cancellation-request" => Some(PrimitiveInfoKind::CancellationRequest),
        b"conduit/terminal-info@1" => Some(PrimitiveInfoKind::Terminal),
        b"value/bool" => Some(PrimitiveInfoKind::Bool),
        b"value/count" => Some(PrimitiveInfoKind::Count),
        b"value/scalar" => Some(PrimitiveInfoKind::Scalar),
        b"value/text" => Some(PrimitiveInfoKind::Text),
        b"value/bytes" => Some(PrimitiveInfoKind::Bytes),
        b"value/quantity@1" => Some(PrimitiveInfoKind::Quantity),
        b"value/unit@1" => Some(PrimitiveInfoKind::Unit),
        b"value/u8" => Some(PrimitiveInfoKind::U8),
        b"value/u16" => Some(PrimitiveInfoKind::U16),
        b"value/u32" => Some(PrimitiveInfoKind::U32),
        b"value/u64" => Some(PrimitiveInfoKind::U64),
        b"value/u128" => Some(PrimitiveInfoKind::U128),
        b"value/i8" => Some(PrimitiveInfoKind::I8),
        b"value/i16" => Some(PrimitiveInfoKind::I16),
        b"value/i32" => Some(PrimitiveInfoKind::I32),
        b"value/i64" => Some(PrimitiveInfoKind::I64),
        b"value/i128" => Some(PrimitiveInfoKind::I128),
        b"value/ieee754-binary32" => Some(PrimitiveInfoKind::F32),
        b"value/ieee754-binary64" => Some(PrimitiveInfoKind::F64),
        _ => None,
    }
}

pub fn validate_primitive_info(identity: &str, encoded: &[u8]) -> Result<(), PrimitiveInfoRefusal> {
    if let Some((family, role)) = crate::parse_quantity_role_info_id(identity) {
        let quantity = Quantity::decode(encoded).map_err(PrimitiveInfoRefusal::Quantity)?;
        if quantity.family().identity() != family {
            return Err(PrimitiveInfoRefusal::WrongQuantityFamily);
        }
        if quantity.role() != role {
            return Err(PrimitiveInfoRefusal::WrongQuantityRole);
        }
        return Ok(());
    }
    match primitive_info_kind(identity) {
        Some(PrimitiveInfoKind::Bytes) if encoded.len() > MAXIMUM_BYTES_INFO_BYTES => {
            Err(PrimitiveInfoRefusal::BytesTooLarge {
                maximum: MAXIMUM_BYTES_INFO_BYTES,
                actual: encoded.len(),
            })
        }
        Some(PrimitiveInfoKind::Empty) if !encoded.is_empty() => {
            Err(PrimitiveInfoRefusal::EmptyNotEmpty)
        }
        Some(PrimitiveInfoKind::CancellationRequest) if !encoded.is_empty() => {
            Err(PrimitiveInfoRefusal::EmptyNotEmpty)
        }
        Some(PrimitiveInfoKind::Terminal) => TerminalInfo::decode(encoded)
            .map(|_| ())
            .map_err(PrimitiveInfoRefusal::Terminal),
        Some(
            PrimitiveInfoKind::Empty
            | PrimitiveInfoKind::CancellationRequest
            | PrimitiveInfoKind::Bytes,
        )
        | None => Ok(()),
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
        Some(PrimitiveInfoKind::Unit) => crate::Unit::decode(encoded)
            .map(|_| ())
            .map_err(PrimitiveInfoRefusal::Unit),
        Some(
            PrimitiveInfoKind::Distance
            | PrimitiveInfoKind::Frequency
            | PrimitiveInfoKind::Duration
            | PrimitiveInfoKind::Voltage
            | PrimitiveInfoKind::Temperature
            | PrimitiveInfoKind::Angle
            | PrimitiveInfoKind::Ratio
            | PrimitiveInfoKind::PixelCount,
        ) => {
            let quantity = Quantity::decode(encoded).map_err(PrimitiveInfoRefusal::Quantity)?;
            let expected = match primitive_info_kind(identity) {
                Some(PrimitiveInfoKind::Distance) => QuantityDimension::Length,
                Some(PrimitiveInfoKind::Frequency) => QuantityDimension::Frequency,
                Some(PrimitiveInfoKind::Duration) => QuantityDimension::Time,
                Some(PrimitiveInfoKind::Voltage) => QuantityDimension::Voltage,
                Some(PrimitiveInfoKind::Temperature) => QuantityDimension::Temperature,
                Some(PrimitiveInfoKind::Angle) => QuantityDimension::Angle,
                Some(PrimitiveInfoKind::Ratio) => QuantityDimension::Ratio,
                Some(PrimitiveInfoKind::PixelCount) => QuantityDimension::PixelCount,
                _ => unreachable!("matched dimensioned quantity kind"),
            };
            let actual = quantity.dimension();
            if actual == expected {
                Ok(())
            } else {
                Err(PrimitiveInfoRefusal::WrongQuantityDimension { expected, actual })
            }
        }
        Some(kind @ (PrimitiveInfoKind::F32 | PrimitiveInfoKind::F64)) => {
            let expected = match kind {
                PrimitiveInfoKind::F32 => crate::IeeeF32::ENCODED_LEN,
                PrimitiveInfoKind::F64 => crate::IeeeF64::ENCODED_LEN,
                _ => unreachable!("matched float kind"),
            };
            if encoded.len() == expected {
                Ok(())
            } else {
                Err(PrimitiveInfoRefusal::FloatLength {
                    expected,
                    actual: encoded.len(),
                })
            }
        }
        Some(
            kind @ (PrimitiveInfoKind::U8
            | PrimitiveInfoKind::U16
            | PrimitiveInfoKind::U32
            | PrimitiveInfoKind::U64
            | PrimitiveInfoKind::U128
            | PrimitiveInfoKind::I8
            | PrimitiveInfoKind::I16
            | PrimitiveInfoKind::I32
            | PrimitiveInfoKind::I64
            | PrimitiveInfoKind::I128),
        ) => {
            let expected = fixed_integer_bytes(kind);
            if encoded.len() == expected {
                Ok(())
            } else {
                Err(PrimitiveInfoRefusal::IntegerLength {
                    expected,
                    actual: encoded.len(),
                })
            }
        }
    }
}

pub const fn fixed_integer_bytes(kind: PrimitiveInfoKind) -> usize {
    match kind {
        PrimitiveInfoKind::U8 | PrimitiveInfoKind::I8 => 1,
        PrimitiveInfoKind::U16 | PrimitiveInfoKind::I16 => 2,
        PrimitiveInfoKind::U32 | PrimitiveInfoKind::I32 => 4,
        PrimitiveInfoKind::U64 | PrimitiveInfoKind::I64 => 8,
        PrimitiveInfoKind::U128 | PrimitiveInfoKind::I128 => 16,
        _ => 0,
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

/// The empty product value, encoded with no payload bytes.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InfoEmpty;
impl InfoEmpty {
    pub const fn encode(self) -> [u8; 0] {
        []
    }
    pub fn decode(encoded: &[u8]) -> Result<Self, PrimitiveInfoRefusal> {
        if encoded.is_empty() {
            Ok(Self)
        } else {
            Err(PrimitiveInfoRefusal::EmptyNotEmpty)
        }
    }
}
