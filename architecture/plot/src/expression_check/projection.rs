//! Exact finite member selection for pure expressions.
use super::*;

pub(super) fn check(
    source: &CheckedExpressionType,
    member: &ExpressionProjection,
    span: Span,
    context: &ExpressionTypeContext<'_>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    if let (CheckedExpressionType::Semantic(kind), ExpressionProjection::Field(field)) =
        (source, member)
    {
        if let Some(ty) = context.structured_types.get(kind) {
            if let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() {
                return cases
                    .iter()
                    .find(|case| case.tag() == field.text)
                    .map(|case| structures::member(case.payload_type()))
                    .ok_or_else(|| diagnostic(field.span, "variant has no such case"));
            }
        }
    }
    if let (CheckedExpressionType::Semantic(kind), ExpressionProjection::TupleIndex(index)) =
        (source, member)
    {
        if let Some(ty) = context.structured_types.get(kind) {
            if let StructuredInfoTypeShape::Sequence {
                element,
                maximum_items,
                ..
            } = ty.shape()
            {
                return index
                    .text
                    .parse::<u16>()
                    .ok()
                    .filter(|index| *index < maximum_items)
                    .map(|_| structures::member(element))
                    .ok_or_else(|| {
                        diagnostic(index.span, "sequence index is outside the admitted maximum")
                    });
            }
        }
    }
    let expanded = match source {
        CheckedExpressionType::Semantic(kind) => context
            .structured_types
            .get(kind)
            .map(CheckedExpressionType::from_structured)
            .unwrap_or_else(|| source.clone()),
        _ => source.clone(),
    };
    match (expanded, member) {
        (CheckedExpressionType::Record(fields), ExpressionProjection::Field(field)) => fields
            .into_iter()
            .find(|(name, _)| name == &field.text)
            .map(|(_, value_type)| value_type)
            .ok_or_else(|| diagnostic(field.span, "record has no such field")),
        (CheckedExpressionType::Tuple(values), ExpressionProjection::TupleIndex(index)) => index
            .text
            .parse::<usize>()
            .ok()
            .and_then(|index| values.get(index).cloned())
            .ok_or_else(|| diagnostic(index.span, "tuple index is outside the exact tuple")),
        (
            CheckedExpressionType::Collection { element, length },
            ExpressionProjection::TupleIndex(index),
        ) => index
            .text
            .parse::<u16>()
            .ok()
            .filter(|index| *index < length)
            .map(|_| *element)
            .ok_or_else(|| {
                diagnostic(
                    index.span,
                    "collection index is outside the exact collection",
                )
            }),
        _ => refuse(span, "projection does not match the exact input type"),
    }
}
