//! Finite encoded extent of reviewed primitive representations.
use conduit_core::PrimitiveInfoKind;

pub(super) fn intrinsic_maximum_bytes(value_kind: &str) -> Option<u32> {
    match conduit_core::primitive_info_kind(value_kind)? {
        PrimitiveInfoKind::Empty | PrimitiveInfoKind::CancellationRequest => Some(0),
        PrimitiveInfoKind::Bool => Some(conduit_core::BOOL_ENCODED_LEN as u32),
        PrimitiveInfoKind::Count => Some(conduit_core::COUNT_ENCODED_LEN as u32),
        PrimitiveInfoKind::Scalar => Some(conduit_core::SCALAR_ENCODED_LEN as u32),
        PrimitiveInfoKind::Quantity
        | PrimitiveInfoKind::Distance
        | PrimitiveInfoKind::Frequency
        | PrimitiveInfoKind::Duration
        | PrimitiveInfoKind::Voltage
        | PrimitiveInfoKind::Temperature
        | PrimitiveInfoKind::Angle
        | PrimitiveInfoKind::Ratio
        | PrimitiveInfoKind::PixelCount => {
            Some(conduit_core::QUANTITY_VALUE_MAX_ENCODED_BYTES as u32)
        }
        PrimitiveInfoKind::Unit => Some(conduit_core::UNIT_ENCODED_LEN as u32),
        PrimitiveInfoKind::MathematicalValue => {
            Some(conduit_core::MATHEMATICAL_VALUE_MAX_ENCODED_BYTES as u32)
        }
        PrimitiveInfoKind::MathematicalAngle => {
            Some(conduit_core::MATHEMATICAL_ANGLE_MAX_ENCODED_BYTES as u32)
        }
        PrimitiveInfoKind::Dimension => Some(conduit_core::DIMENSION_ENCODED_LEN as u32),
        kind @ (PrimitiveInfoKind::U8
        | PrimitiveInfoKind::U16
        | PrimitiveInfoKind::U32
        | PrimitiveInfoKind::U64
        | PrimitiveInfoKind::U128
        | PrimitiveInfoKind::I8
        | PrimitiveInfoKind::I16
        | PrimitiveInfoKind::I32
        | PrimitiveInfoKind::I64
        | PrimitiveInfoKind::I128) => Some(conduit_core::fixed_integer_bytes(kind) as u32),
        PrimitiveInfoKind::F32 => Some(conduit_core::IeeeF32::ENCODED_LEN as u32),
        PrimitiveInfoKind::F64 => Some(conduit_core::IeeeF64::ENCODED_LEN as u32),
        PrimitiveInfoKind::Text | PrimitiveInfoKind::Bytes | PrimitiveInfoKind::Terminal => None,
    }
}
