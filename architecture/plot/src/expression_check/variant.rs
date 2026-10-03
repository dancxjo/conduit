//! Construction of one declared case of an exact finite variant.
use super::*;

pub(super) fn check(
    tag: &crate::SpannedText,
    payload: &ExpressionSyntax,
    span: Span,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
    node_types: &mut Vec<CheckedExpressionNodeType>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    let kind = expected
        .and_then(CheckedExpressionType::value_kind)
        .ok_or_else(|| {
            diagnostic(
                span,
                "variant expression requires an exact expected variant type",
            )
        })?;
    let ty = context
        .structured_types
        .get(kind)
        .ok_or_else(|| diagnostic(span, "variant expression requires a declared schema"))?;
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        return refuse(
            span,
            "variant expression does not match the exact expected type",
        );
    };
    let case = cases
        .iter()
        .find(|case| case.tag() == tag.text)
        .ok_or_else(|| diagnostic(tag.span, "variant has no such case"))?;
    let payload_type = structures::member(case.payload_type());
    let actual = infer(payload, Some(&payload_type), context, node_types)?;
    require(
        actual,
        &payload_type,
        payload.span(),
        "variant payload must have its exact declared type",
    )?;
    Ok(expected.expect("exact variant").clone())
}
