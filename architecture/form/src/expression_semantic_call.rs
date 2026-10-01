use crate::prelude::*;
use crate::{
    CheckedExpressionType, ExpressionSyntax, ExpressionTypeContext, ExpressionTypeDiagnostic, Span,
    SpannedText,
};
use conduit_core::{Kind, PortTemporal};

pub(super) fn check(
    name: &SpannedText,
    arguments: &[ExpressionSyntax],
    span: Span,
    context: &ExpressionTypeContext<'_>,
    mut check_argument: impl FnMut(
        &ExpressionSyntax,
        &CheckedExpressionType,
    ) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    let kind = context
        .semantic_kinds
        .get(&name.text)
        .ok_or_else(|| diagnostic(name.span, "semantic call names an unknown Kind"))?;
    conduit_core::pure_expression_facts(kind).map_err(|refusal| {
        diagnostic(
            name.span,
            &format!("semantic Kind is ineligible for pure expression use: {refusal:?}"),
        )
    })?;
    check_front(kind, arguments.len(), span)?;
    for (argument, input) in arguments.iter().zip(&kind.inputs) {
        let expected = CheckedExpressionType::Semantic(input.value_kind.clone());
        let actual = check_argument(argument, &expected)?;
        if actual != expected {
            return Err(diagnostic(
                argument.span(),
                "semantic call argument violates its exact input Port type",
            ));
        }
    }
    Ok(CheckedExpressionType::Semantic(
        kind.outputs[0].value_kind.clone(),
    ))
}

fn check_front(
    kind: &Kind,
    argument_count: usize,
    span: Span,
) -> Result<(), ExpressionTypeDiagnostic> {
    if kind
        .startup_parameters
        .iter()
        .any(|field| !field.has_default)
    {
        return Err(diagnostic(
            span,
            "semantic call Kind has required startup configuration",
        ));
    }
    if kind.inputs.len() != argument_count {
        return Err(diagnostic(
            span,
            "semantic call argument count does not match its exact Front",
        ));
    }
    if kind.outputs.len() != 1 {
        return Err(diagnostic(
            span,
            "pure semantic call must produce exactly one value",
        ));
    }
    if kind
        .inputs
        .iter()
        .chain(&kind.outputs)
        .any(|port| port.temporal != PortTemporal::Value)
    {
        return Err(diagnostic(
            span,
            "pure semantic call Ports must use one-value temporal contracts",
        ));
    }
    Ok(())
}

fn diagnostic(span: Span, message: &str) -> ExpressionTypeDiagnostic {
    ExpressionTypeDiagnostic {
        span,
        message: message.to_string(),
    }
}
