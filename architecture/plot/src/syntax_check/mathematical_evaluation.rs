//! Retain ordered source computation before startup denotation normalization.
use super::*;

pub(super) fn capture(
    invocation: &Invocation,
    catalog: &StartupCatalog,
) -> Result<Option<(conduit_core::NumericComputation, [u8; 32])>, SyntaxCheckDiagnostic> {
    if invocation.kind.text != "math/evaluate" {
        return Ok(None);
    }
    let value = invocation
        .arguments
        .iter()
        .find_map(|argument| match argument {
            Argument::Named { name, value, .. } if name.text == "value" => Some(value),
            Argument::Positional(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| SyntaxCheckDiagnostic {
            code: "CND-FRM-053",
            span: invocation.span,
            message: "math/evaluate requires an authored computation".into(),
        })?;
    let graph = crate::expression_scalar::checked_closed_numeric_expression_with_catalog(&value.syntax, Some(catalog))
        .map_err(|message| SyntaxCheckDiagnostic {
            code: "CND-FRM-053",
            span: value.span,
            message,
        })?;
    if graph.nodes().iter().any(|node| {
        matches!(
            node.operation,
            conduit_core::NumericComputationOperation::Parameter(_)
        )
    }) {
        return Err(SyntaxCheckDiagnostic { code: "CND-FRM-053", span: value.span,
            message: "math/evaluate startup computation must be closed; runtime parameters require explicit checked graph bindings".into() });
    }
    let source_identity = conduit_core::semantic_digest(
        "conduit.numeric.source-computation.v1",
        value.text.as_bytes(),
    );
    Ok(Some((graph, source_identity)))
}
