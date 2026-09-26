use crate::{CheckedExpressionType, ExpressionTypeContext};

pub(super) fn boolean() -> CheckedExpressionType {
    CheckedExpressionType::semantic("value/bool")
}

pub(super) fn is_fixed_integer(value_type: &CheckedExpressionType) -> bool {
    value_type.value_kind().is_some_and(|kind| {
        matches!(
            kind.as_str(),
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

pub(super) fn is_signed_numeric(value_type: &CheckedExpressionType) -> bool {
    value_type.value_kind().is_some_and(|kind| {
        matches!(
            kind.as_str(),
            "value/i8" | "value/i16" | "value/i32" | "value/i64" | "value/i128" | "value/scalar"
        )
    })
}

pub(super) fn is_numeric(
    value_type: &CheckedExpressionType,
    context: &ExpressionTypeContext<'_>,
) -> bool {
    is_fixed_integer(value_type)
        || value_type.value_kind().is_some_and(|kind| {
            matches!(kind.as_str(), "value/count" | "value/scalar")
                || kind.as_str() == conduit_core::QUANTITY_INFO_ID
                || conduit_core::quantity_info_dimension(kind.as_str()).is_some()
                || context.numeric_types.contains(kind)
        })
}

pub(super) fn is_ordered_numeric(
    value_type: &CheckedExpressionType,
    context: &ExpressionTypeContext<'_>,
) -> bool {
    is_numeric(value_type, context)
}
