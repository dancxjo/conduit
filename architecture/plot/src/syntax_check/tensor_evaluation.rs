//! Preserve each original ordered mathematical coordinate before folding startup values.
use super::*;
use crate::ExpressionSyntax;

pub(super) fn capture(
    invocation: &Invocation,
    catalog: &StartupCatalog,
) -> Result<Option<crate::CapturedTensorComputations>, SyntaxCheckDiagnostic> {
    if invocation.kind.text != "tensor/evaluate" {
        return Ok(None);
    }
    let fail = |message: &str| SyntaxCheckDiagnostic {
        code: "CND-FRM-053",
        span: invocation.span,
        message: message.into(),
    };
    let value = invocation
        .arguments
        .iter()
        .find_map(|argument| match argument {
            Argument::Named { name, value, .. } if name.text == "values" => Some(value),
            Argument::Positional(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| fail("tensor/evaluate requires authored mathematical coordinates"))?;
    let ExpressionSyntax::Collection { values, .. } = &value.syntax else {
        return Err(fail(
            "tensor/evaluate coordinates must be a finite authored collection",
        ));
    };
    if values.is_empty() || values.len() > 8 {
        return Err(fail("tensor/evaluate supports one to eight coordinates"));
    }
    let source =
        conduit_core::semantic_digest("conduit.tensor.source-coordinates@1", value.text.as_bytes());
    let mut captured = Vec::with_capacity(values.len());
    for (index, expression) in values.iter().enumerate() {
        let graph = crate::expression_scalar::checked_closed_numeric_expression_with_catalog(
            expression,
            Some(catalog),
        )
        .map_err(|message| fail(&message))?;
        if graph.nodes().iter().any(|node| {
            matches!(
                node.operation,
                conduit_core::NumericComputationOperation::Parameter(_)
            )
        }) || (graph.nodes()[graph.root() as usize].semantic_kind
            != conduit_core::MATH_VALUE_INFO_ID
            && conduit_core::PortableNumericProfile::from_info_id(
                &graph.nodes()[graph.root() as usize].semantic_kind,
            )
            .is_none())
        {
            return Err(fail(
                "tensor/evaluate requires closed mathematical Scalar coordinates",
            ));
        }
        let mut ordinal_source = source.to_vec();
        ordinal_source.extend_from_slice(&(index as u16).to_be_bytes());
        let identity =
            conduit_core::semantic_digest("conduit.tensor.coordinate-source@1", &ordinal_source);
        captured.push((graph, identity));
    }
    Ok(Some(captured))
}
