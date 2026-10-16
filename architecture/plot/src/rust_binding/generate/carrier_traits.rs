//! Native carrier derivation follows representation mechanics, not mathematical equality.
use conduit_core::{PrimitiveInfoKind, StructuredInfoType, StructuredInfoTypeShape};

pub(crate) fn copy_type(value_type: &StructuredInfoType) -> bool {
    match value_type.shape() {
        StructuredInfoTypeShape::Leaf(kind) => !matches!(
            conduit_core::primitive_info_kind(kind.as_str()),
            Some(
                PrimitiveInfoKind::Text
                    | PrimitiveInfoKind::Bytes
                    | PrimitiveInfoKind::MathematicalValue
                    | PrimitiveInfoKind::MathematicalAngle
            ) | None
        ),
        StructuredInfoTypeShape::Nominal { representation, .. } => copy_type(representation),
        StructuredInfoTypeShape::Record { fields, .. } => {
            fields.iter().all(|field| copy_type(field.value_type()))
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            cases.iter().all(|case| copy_type(case.payload_type()))
        }
        StructuredInfoTypeShape::Sequence { .. } => false,
        StructuredInfoTypeShape::Collection { element, .. } => copy_type(element),
    }
}

pub(super) fn hash_type(value_type: &StructuredInfoType) -> bool {
    match value_type.shape() {
        StructuredInfoTypeShape::Leaf(kind) => matches!(
            conduit_core::primitive_info_kind(kind.as_str()),
            Some(
                PrimitiveInfoKind::Empty
                    | PrimitiveInfoKind::CancellationRequest
                    | PrimitiveInfoKind::Bool
                    | PrimitiveInfoKind::Count
                    | PrimitiveInfoKind::Text
                    | PrimitiveInfoKind::I8
                    | PrimitiveInfoKind::U8
                    | PrimitiveInfoKind::I16
                    | PrimitiveInfoKind::U16
                    | PrimitiveInfoKind::I32
                    | PrimitiveInfoKind::U32
                    | PrimitiveInfoKind::I64
                    | PrimitiveInfoKind::U64
                    | PrimitiveInfoKind::I128
                    | PrimitiveInfoKind::U128
                    | PrimitiveInfoKind::F32
                    | PrimitiveInfoKind::F64
            )
        ),
        StructuredInfoTypeShape::Nominal { representation, .. } => hash_type(representation),
        StructuredInfoTypeShape::Collection { element, .. } => hash_type(element),
        StructuredInfoTypeShape::Record { .. }
        | StructuredInfoTypeShape::Variant { .. }
        | StructuredInfoTypeShape::Sequence { .. } => false,
    }
}
