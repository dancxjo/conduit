//! Explicit physical carrier widening. Original wire and Unit/role stay intact.
use crate::{CheckedExpressionType, ExpressionSyntax, ExpressionTypeDiagnostic, Span};
pub(crate) const CALL: &str = "quantity/value";
pub(crate) fn check(
    arguments: &[ExpressionSyntax],
    span: Span,
    mut check_argument: impl FnMut(
        &ExpressionSyntax,
        Option<&CheckedExpressionType>,
    ) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    let [argument] = arguments else {
        return Err(ExpressionTypeDiagnostic {
            span,
            message: "quantity/value requires one physical Quantity".into(),
        });
    };
    let ty = check_argument(argument, None)?;
    if !ty
        .value_kind()
        .is_some_and(|kind| crate::expression_quantity::is_quantity(kind.as_str()))
    {
        return Err(ExpressionTypeDiagnostic { span, message: "quantity/value requires checked physical family and role; bare scalars have no implicit Unit".into() });
    }
    Ok(CheckedExpressionType::semantic(
        conduit_core::QUANTITY_INFO_ID,
    ))
}
