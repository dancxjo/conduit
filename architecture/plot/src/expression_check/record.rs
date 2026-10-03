//! Contextual construction of records with an exact declared schema.
use super::*;

pub(super) fn record(
    fields: &[crate::StructuredExpressionField],
    span: Span,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
    node_types: &mut Vec<CheckedExpressionNodeType>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    let named = expected
        .and_then(|ty| ty.value_kind())
        .and_then(|kind| context.structured_types.get(kind));
    let expanded = named.map(CheckedExpressionType::from_structured);
    let expected_shape = expanded.as_ref().or(expected);
    let mut checked = Vec::with_capacity(fields.len());
    for field in fields {
        if checked
            .iter()
            .any(|(name, _): &(String, CheckedExpressionType)| name == &field.name.text)
        {
            return refuse(field.name.span, "record field names must be unique");
        }
        let expected_field = match expected_shape {
            Some(CheckedExpressionType::Record(fields)) => fields
                .iter()
                .find(|(name, _)| name == &field.name.text)
                .map(|(_, value_type)| value_type),
            _ => None,
        };
        checked.push((
            field.name.text.clone(),
            infer(&field.value, expected_field, context, node_types)?,
        ));
    }
    if checked.is_empty() {
        return refuse(span, "record must contain at least one field");
    }
    checked.sort_by(|left, right| left.0.cmp(&right.0));
    let actual = CheckedExpressionType::Record(checked);
    if let Some(expected_shape) = expected_shape {
        require(
            actual.clone(),
            expected_shape,
            span,
            "record must contain the exact declared fields and types",
        )?;
    }
    Ok(if named.is_some() {
        expected.expect("named expected type").clone()
    } else {
        actual
    })
}
