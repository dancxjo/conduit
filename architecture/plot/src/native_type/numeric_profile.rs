//! Intrinsic numerical bound selection for authored primitive representations.
use super::*;

pub(super) fn bounded_representation(
    value: StructuredInfoType,
    maximum: Option<u64>,
    span: crate::Span,
) -> Result<StructuredInfoType, SyntaxCheckDiagnostic> {
    if let conduit_core::StructuredInfoTypeShape::Nominal {
        schema,
        representation,
    } = value.shape()
    {
        let representation = bounded_representation(representation.clone(), maximum, span)?;
        return StructuredInfoType::nominal(schema.clone(), representation)
            .map_err(|reason| bounded(span, reason));
    }
    let conduit_core::StructuredInfoTypeShape::Leaf(kind) = value.shape() else {
        return Ok(value);
    };
    let kind = crate::value_type::numeric_profile::bounded_kind(kind, maximum, span)?;
    StructuredInfoType::leaf(kind).map_err(|reason| bounded(span, reason))
}

pub(super) fn primitive_representation_kind(value_type: &StructuredInfoType) -> Option<&KindId> {
    match value_type.shape() {
        conduit_core::StructuredInfoTypeShape::Leaf(kind) => Some(kind),
        conduit_core::StructuredInfoTypeShape::Nominal { representation, .. } => {
            primitive_representation_kind(representation)
        }
        _ => None,
    }
}
