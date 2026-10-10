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
                "integer conversion must widen while representing the entire source domain",
            ));
        }
        return Ok(CheckedExpressionType::semantic(target));
    }
    if matches!(name.text.as_str(), "bytes/length" | "bytes/at") {
        let expected_count = if name.text == "bytes/at" { 2 } else { 1 };
        if arguments.len() != expected_count {
            return Err(diagnostic(
                span,
                "byte observation requires Bytes and, for indexing, a U64 index",
            ));
        }
        let bytes = CheckedExpressionType::semantic("value/bytes");
        if check_argument(&arguments[0], None)? != bytes {
            return Err(diagnostic(
                arguments[0].span(),
                "byte observation requires exact Bytes",
            ));
        }
        if name.text == "bytes/at" {
            let index = CheckedExpressionType::semantic("value/u64");
            if check_argument(&arguments[1], Some(&index))? != index {
                return Err(diagnostic(
                    arguments[1].span(),
                    "byte index requires exact U64",
                ));
            }
            return Ok(CheckedExpressionType::semantic("value/u8"));
        }
        return Ok(CheckedExpressionType::semantic("value/u64"));
    }
    if name.text == "sequence/at" {
        let [source, index] = arguments else {
            return Err(diagnostic(
                span,
                "sequence selection requires a collection and U64 index",
            ));
        };
        let source_type = check_argument(source, None)?;
        let expected_index = CheckedExpressionType::semantic("value/u64");
        if check_argument(index, Some(&expected_index))? != expected_index {
            return Err(diagnostic(
                index.span(),
                "sequence index requires exact U64",
            ));
        }
        let ty = source_type
            .structured_info_type_with(context.structured_types)
            .map_err(|_| {
                diagnostic(
                    source.span(),
                    "sequence selection requires an exact finite collection Type",
                )
            })?;
        let element = collection_element(&ty).ok_or_else(|| {
            diagnostic(
                source.span(),
                "sequence selection requires an exact finite collection Type",
            )
        })?;
        return Ok(CheckedExpressionType::from_member(element));
    }
    if name.text == "sequence/length" {
        let [argument] = arguments else {
            return Err(diagnostic(
                span,
                "sequence length requires exactly one value",
            ));
        };
        let source = check_argument(argument, None)?;
        let ty = source
            .structured_info_type_with(context.structured_types)
            .map_err(|_| {
                diagnostic(
                    argument.span(),
                    "sequence length requires an exact finite collection Type",
                )
            })?;
        if collection_element(&ty).is_none() {
            return Err(diagnostic(
                argument.span(),
                "sequence length requires an exact finite collection Type",
            ));
        }
        return Ok(CheckedExpressionType::semantic("value/u64"));
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
    if name.text == "variant/is" {
        if arguments.len() != 2 {
            return Err(diagnostic(
                span,
                "variant case test requires one value and one case",
            ));
        }
        let source = check_argument(&arguments[0], None)?;
        let Some(kind) = source.value_kind() else {
            return Err(diagnostic(
                arguments[0].span(),
                "case test requires a semantic variant",
            ));
        };
        let Some(value_type) = context.structured_types.get(kind) else {
            return Err(diagnostic(
                arguments[0].span(),
                "case test requires a semantic variant",
            ));
        };
        let conduit_core::StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape()
        else {
            return Err(diagnostic(
                arguments[0].span(),
                "case test requires a semantic variant",
            ));
        };
        let ExpressionSyntax::Atomic(case) = &arguments[1] else {
            return Err(diagnostic(
                arguments[1].span(),
                "case test requires one declared case",
            ));
        };
        let Some(case_name) = crate::text_value::parse_quoted_text(&case.text) else {
            return Err(diagnostic(
                case.span,
                "case test requires one declared case",
            ));
        };
        if !cases.iter().any(|candidate| candidate.tag() == case_name) {
            return Err(diagnostic(
                case.span,
                "case is not declared by this exact variant Type",
            ));
        }
        check_argument(
            &arguments[1],
            Some(&CheckedExpressionType::semantic(conduit_core::TEXT_INFO_ID)),
        )?;
        return Ok(CheckedExpressionType::semantic(conduit_core::BOOL_INFO_ID));
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

/// Observe collection representation without changing its or its elements' Type.
pub(crate) fn collection_element(
    value_type: &conduit_core::StructuredInfoType,
) -> Option<&conduit_core::StructuredInfoType> {
    let mut value_type = value_type;
    for _ in 0..=conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH {
        match value_type.shape() {
            conduit_core::StructuredInfoTypeShape::Nominal { representation, .. } => {
                value_type = representation;
            }
            conduit_core::StructuredInfoTypeShape::Collection { element, .. }
            | conduit_core::StructuredInfoTypeShape::Sequence { element, .. } => {
                return Some(element)
            }
            _ => return None,
        }
    }
    None
}

pub(crate) fn integer_widening_target(name: &str) -> Option<&str> {
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

pub(crate) fn is_strict_widening(source: &str, target: &str) -> bool {
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
    matches!((width(source), width(target)), (Some((source_signed, source_width)), Some((target_signed, target_width))) if source_width < target_width && (source_signed == target_signed || target_signed))
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

pub(crate) fn is_intrinsic(name: &str) -> bool {
    integer_widening_target(name).is_some()
        || matches!(
            name,
            "variant/tag"
                | "variant/is"
                | "sequence/length"
                | "sequence/at"
                | "bytes/length"
                | "bytes/at"
        )
}
