//! Primitive semantic leaf to native carrier mapping.
use super::*;

pub(crate) fn primitive_rust_type(identity: &str) -> Result<String, RustBindingGenerationError> {
    let value = match conduit_core::primitive_info_kind(identity) {
        Some(PrimitiveInfoKind::Empty | PrimitiveInfoKind::CancellationRequest) => "()",
        Some(PrimitiveInfoKind::Bool) => "bool",
        Some(PrimitiveInfoKind::Count) => "u64",
        Some(PrimitiveInfoKind::Scalar) => "conduit_core::Scalar",
        Some(PrimitiveInfoKind::Text) => "String",
        Some(PrimitiveInfoKind::Bytes) | None
            if identity.starts_with("data/generation-reference<") =>
        {
            "conduit_data::DataReference"
        }
        None if identity == conduit_core::RESOURCE_REFERENCE_INFO_ID => {
            "conduit_core::BoundedResourceRef"
        }
        // A generated primitive carrier must be able to retain every byte
        // value admitted by the checked semantic contract. Per-field and
        // nominal limits are still enforced by generated value contracts;
        // this binding capacity is the structured substrate's finite ceiling,
        // not an additional 4 KiB semantic restriction.
        Some(PrimitiveInfoKind::Bytes) => "BoundedBytes<65536>",
        Some(
            PrimitiveInfoKind::Quantity
            | PrimitiveInfoKind::Distance
            | PrimitiveInfoKind::Frequency
            | PrimitiveInfoKind::Duration
            | PrimitiveInfoKind::Voltage
            | PrimitiveInfoKind::Temperature
            | PrimitiveInfoKind::Angle
            | PrimitiveInfoKind::Ratio
            | PrimitiveInfoKind::PixelCount,
        ) => "conduit_core::Quantity",
        Some(PrimitiveInfoKind::Unit) => "conduit_core::Unit",
        Some(PrimitiveInfoKind::Dimension) => "conduit_core::Dimension",
        Some(PrimitiveInfoKind::MathematicalValue) => "conduit_core::MathematicalValue",
        Some(PrimitiveInfoKind::MathematicalAngle) => "conduit_core::MathematicalAngle",
        Some(PrimitiveInfoKind::U8) => "u8",
        Some(PrimitiveInfoKind::U16) => "u16",
        Some(PrimitiveInfoKind::U32) => "u32",
        Some(PrimitiveInfoKind::U64) => "u64",
        Some(PrimitiveInfoKind::U128) => "u128",
        Some(PrimitiveInfoKind::I8) => "i8",
        Some(PrimitiveInfoKind::I16) => "i16",
        Some(PrimitiveInfoKind::I32) => "i32",
        Some(PrimitiveInfoKind::I64) => "i64",
        Some(PrimitiveInfoKind::I128) => "i128",
        Some(PrimitiveInfoKind::F32) => "conduit_core::IeeeF32",
        Some(PrimitiveInfoKind::F64) => "conduit_core::IeeeF64",
        Some(PrimitiveInfoKind::Terminal) | None => {
            return Err(RustBindingGenerationError::UnsupportedLeaf(identity.into()));
        }
    };
    Ok(value.into())
}

pub(crate) fn data_reference_content_kind(identity: &str) -> Option<&str> {
    identity
        .strip_prefix("data/generation-reference<")
        .and_then(|identity| identity.strip_suffix('>'))
        .filter(|identity| !identity.is_empty())
}

pub(crate) fn unit_type(value_type: &StructuredInfoType) -> bool {
    matches!(value_type.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == conduit_core::EMPTY_INFO_ID)
}
