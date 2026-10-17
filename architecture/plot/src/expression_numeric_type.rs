use crate::{CheckedExpressionType, ExpressionTypeContext};

pub(super) fn boolean() -> CheckedExpressionType {
    CheckedExpressionType::semantic("value/bool")
}

fn represented_kind<'a>(
    value_type: &'a CheckedExpressionType,
    context: &'a ExpressionTypeContext<'_>,
) -> Option<&'a str> {
    let kind = value_type.value_kind()?;
    let Some(structured) = context.structured_types.get(kind) else {
        return Some(kind.as_str());
    };
    match structured.shape() {
        conduit_core::StructuredInfoTypeShape::Nominal { representation, .. } => {
            match representation.shape() {
                conduit_core::StructuredInfoTypeShape::Leaf(kind) => Some(kind.as_str()),
                _ => Some(kind.as_str()),
            }
        }
        _ => Some(kind.as_str()),
    }
}

pub(super) fn is_fixed_integer(
    value_type: &CheckedExpressionType,
    context: &ExpressionTypeContext<'_>,
) -> bool {
    represented_kind(value_type, context).is_some_and(|kind| {
        matches!(
            kind,
            "value/u8"
                | "value/u16"
                | "value/u32"
                | "value/u64"
                | "value/u128"
                | "value/i8"
                | "value/i16"
                | "value/i32"
                | "value/i64"
                | "value/i128"
        )
    })
}

pub(super) fn is_signed_numeric(
    value_type: &CheckedExpressionType,
    context: &ExpressionTypeContext<'_>,
) -> bool {
    represented_kind(value_type, context).is_some_and(|kind| {
        matches!(
            kind,
            "value/i8" | "value/i16" | "value/i32" | "value/i64" | "value/i128" | "value/scalar"
        )
    })
}

pub(super) fn is_numeric(
    value_type: &CheckedExpressionType,
    context: &ExpressionTypeContext<'_>,
) -> bool {
    is_fixed_integer(value_type, context)
        || represented_kind(value_type, context).is_some_and(|kind| {
            matches!(kind, "value/count" | "value/scalar")
                || kind == conduit_core::F32_INFO_ID
                || kind == conduit_core::QUANTITY_INFO_ID
                || conduit_core::quantity_info_dimension(kind).is_some()
                || value_type
                    .value_kind()
                    .is_some_and(|kind| context.numeric_types.contains(kind))
        })
}

pub(super) fn is_ordered_numeric(
    value_type: &CheckedExpressionType,
    context: &ExpressionTypeContext<'_>,
) -> bool {
    is_numeric(value_type, context)
}
