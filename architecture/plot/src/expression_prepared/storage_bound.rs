//! Conservative encoded scratch bounds from the exact checked Type.
use super::Refusal;
use conduit_core::{
    fixed_integer_bytes, primitive_info_kind, PrimitiveInfoKind, StructuredInfoType,
    StructuredInfoTypeShape, MAXIMUM_STRUCTURED_CANONICAL_BYTES, MAXIMUM_STRUCTURED_LEAF_BYTES,
};

pub(super) fn output(ty: &StructuredInfoType) -> Result<usize, Refusal> {
    if let StructuredInfoTypeShape::Leaf(kind) = ty.shape() {
        return Ok(leaf(kind.as_str()));
    }
    canonical(ty)
}
pub(super) fn canonical(ty: &StructuredInfoType) -> Result<usize, Refusal> {
    let prefix = ty
        .canonical_bytes()
        .map_err(|_| Refusal::InvalidProgram)?
        .len();
    Ok(add(prefix, node(ty)).min(MAXIMUM_STRUCTURED_CANONICAL_BYTES))
}
fn node(ty: &StructuredInfoType) -> usize {
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(kind) => add(5, leaf(kind.as_str())),
        StructuredInfoTypeShape::Nominal { representation, .. } => node(representation),
        StructuredInfoTypeShape::Collection { element, length } => {
            add(5, node(element).saturating_mul(usize::from(length)))
        }
        StructuredInfoTypeShape::Sequence {
            element,
            maximum_items,
            ..
        } => add(5, node(element).saturating_mul(usize::from(maximum_items))),
        StructuredInfoTypeShape::Record { fields, .. } => fields.iter().fold(5, |size, field| {
            add(size, add(4 + field.name().len(), node(field.value_type())))
        }),
        StructuredInfoTypeShape::Variant { cases, .. } => add(
            1,
            cases
                .iter()
                .map(|case| add(4 + case.tag().len(), node(case.payload_type())))
                .max()
                .unwrap_or(0),
        ),
    }
    .min(MAXIMUM_STRUCTURED_CANONICAL_BYTES)
}
fn leaf(kind: &str) -> usize {
    let Some(kind) = primitive_info_kind(kind) else {
        return MAXIMUM_STRUCTURED_LEAF_BYTES;
    };
    let fixed = fixed_integer_bytes(kind);
    if fixed != 0 {
        return fixed;
    }
    match kind {
        PrimitiveInfoKind::Unit | PrimitiveInfoKind::CancellationRequest => 0,
        PrimitiveInfoKind::Bool => 1,
        PrimitiveInfoKind::Count => 8,
        PrimitiveInfoKind::Scalar => 16,
        PrimitiveInfoKind::ExactDecimalQuantity => conduit_core::EXACT_DECIMAL_QUANTITY_ENCODED_LEN,
        PrimitiveInfoKind::F32 => 4,
        PrimitiveInfoKind::F64 => 8,
        _ => MAXIMUM_STRUCTURED_LEAF_BYTES,
    }
}
fn add(a: usize, b: usize) -> usize {
    a.saturating_add(b).min(MAXIMUM_STRUCTURED_CANONICAL_BYTES)
}
