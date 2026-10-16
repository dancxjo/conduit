//! Exact syntax reaches an owning resolver; generic structural callers cannot execute calls.
use super::*;
pub(crate) enum StartupValueSyntax<'a> {
    Atomic(&'a SpannedText),
    AngleReference(&'a ExpressionSyntax),
}
pub(crate) fn check_structured_expression(
    expression: &ExpressionSyntax,
    expected: &StructuredInfoType,
    resolve_atomic: &mut dyn FnMut(
        &SpannedText,
        &StructuredInfoType,
    ) -> Result<CanonicalStartupValue, SyntaxCheckDiagnostic>,
) -> Result<CanonicalStructuredStartupValue, SyntaxCheckDiagnostic> {
    super::check_startup_expression(expression, expected, &mut |syntax, expected| match syntax {
        StartupValueSyntax::Atomic(atomic) => resolve_atomic(atomic, expected),
        StartupValueSyntax::AngleReference(call) => Err(structured_diagnostic(
            call.span(),
            "exact Angle reference requires its owning Source resolver",
        )),
    })
}
