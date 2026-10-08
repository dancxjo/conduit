//! Finite collection and bounded-sequence literals in an exact output context.
use super::*;

pub(super) fn check(
    values: &[ExpressionSyntax],
    span: Span,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
    node_types: &mut Vec<CheckedExpressionNodeType>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    let length = u16::try_from(values.len())
        .map_err(|_| diagnostic(span, "collection exceeds its finite length bound"))?;
    let declared = expected
        .and_then(CheckedExpressionType::value_kind)
        .and_then(|kind| context.structured_types.get(kind));
    let representation = declared.map(|mut ty| {
        while let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
            ty = representation;
        }
        ty
    });
    let exact_element = match representation.map(StructuredInfoType::shape) {
        Some(StructuredInfoTypeShape::Sequence {
            element,
            minimum_items,
            maximum_items,
        }) if length >= minimum_items && length <= maximum_items => {
            Some(structures::member(element))
        }
        Some(StructuredInfoTypeShape::Collection { element, length: n }) if length == n => {
            Some(structures::member(element))
        }
        Some(_) => {
            return refuse(
                span,
                "literal length or shape does not match the exact declared collection",
            )
        }
        None => match expected {
            Some(CheckedExpressionType::Collection { element, length: n }) if *n == length => {
                Some(element.as_ref().clone())
            }
            Some(_) => return refuse(span, "literal does not match the exact expected collection"),
            None => None,
        },
    };
    let element = if let Some(first) = values.first() {
        infer(first, exact_element.as_ref(), context, node_types)?
    } else {
        exact_element.clone().ok_or_else(|| {
            diagnostic(
                span,
                "empty collection needs an exact declared element type",
            )
        })?
    };
    if let Some(expected) = &exact_element {
        require(
            element.clone(),
            expected,
            span,
            "collection element must have the exact declared type",
        )?;
    }
    for value in values.iter().skip(1) {
        let actual = infer(value, Some(&element), context, node_types)?;
        require(
            actual,
            &element,
            value.span(),
            "collection elements must have one exact type",
        )?;
    }
    Ok(if declared.is_some() {
        expected.expect("declared collection").clone()
    } else {
        CheckedExpressionType::Collection {
            element: Box::new(element),
            length,
        }
    })
}
