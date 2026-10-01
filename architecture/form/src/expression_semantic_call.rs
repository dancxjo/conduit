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
        Option<&CheckedExpressionType>,
    ) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    if let Some(target) = integer_widening_target(&name.text) {
        if arguments.len() != 1 {
            return Err(diagnostic(
                span,
                "integer widening requires exactly one value",
            ));
        }
        let source = check_argument(&arguments[0], None)?;
        let Some(source_name) = source.value_kind().map(|kind| kind.as_str()) else {
            return Err(diagnostic(
                arguments[0].span(),
                "integer widening requires a fixed integer",
            ));
        };
        if !is_strict_widening(source_name, target) {
            return Err(diagnostic(
                arguments[0].span(),
                "integer conversion must widen without changing signedness",
            ));
        }
        return Ok(CheckedExpressionType::semantic(target));
    }
    if name.text == "variant/tag" {
        if arguments.len() != 1 {
            return Err(diagnostic(span, "variant/tag requires exactly one value"));
        }
        let source = check_argument(&arguments[0], None)?;
        let Some(kind) = source.value_kind() else {
            return Err(diagnostic(
                arguments[0].span(),
                "variant/tag requires a semantic variant",
            ));
        };
        let is_variant = context
            .structured_types
            .get(kind)
            .is_some_and(|value_type| {
                matches!(
                    value_type.shape(),
                    conduit_core::StructuredInfoTypeShape::Variant { .. }
                )
            });
        if !is_variant {
            return Err(diagnostic(
                arguments[0].span(),
                "variant/tag requires a semantic variant",
            ));
        }
        return Ok(CheckedExpressionType::semantic(conduit_core::TEXT_INFO_ID));
    }
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
        let actual = check_argument(argument, Some(&expected))?;
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

fn integer_widening_target(name: &str) -> Option<&str> {
    matches!(
        name,
        "value/u16"
            | "value/u32"
            | "value/u64"
            | "value/u128"
            | "value/i16"
            | "value/i32"
            | "value/i64"
            | "value/i128"
    )
    .then_some(name)
}

fn is_strict_widening(source: &str, target: &str) -> bool {
    fn width(kind: &str) -> Option<(bool, u8)> {
        Some(match kind {
            "value/u8" => (false, 8),
            "value/u16" => (false, 16),
            "value/u32" => (false, 32),
            "value/u64" => (false, 64),
            "value/u128" => (false, 128),
            "value/i8" => (true, 8),
            "value/i16" => (true, 16),
            "value/i32" => (true, 32),
            "value/i64" => (true, 64),
            "value/i128" => (true, 128),
            _ => return None,
        })
    }
    matches!((width(source), width(target)), (Some((source_signed, source_width)), Some((target_signed, target_width))) if source_signed == target_signed && source_width < target_width)
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
