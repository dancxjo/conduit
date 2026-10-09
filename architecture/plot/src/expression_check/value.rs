//! Exact literal, record projection, and qualified unit-variant typing.
use super::*;

pub(super) fn expand(
    value: &CheckedExpressionType,
    context: &ExpressionTypeContext<'_>,
) -> CheckedExpressionType {
    let CheckedExpressionType::Semantic(kind) = value else {
        return value.clone();
    };
    let Some(ty) = context.structured_types.get(kind) else {
        return value.clone();
    };
    if let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() {
        CheckedExpressionType::Record(
            fields
                .iter()
                .map(|field| {
                    let field_type = structures::member(field.value_type());
                    (field.name().into(), field_type)
                })
                .collect(),
        )
    } else {
        CheckedExpressionType::from_structured(ty)
    }
}

pub(super) fn atomic(
    text: &str,
    span: Span,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    if let Some(value_type) = context.immutable_values.get(text) {
        return Ok(value_type.clone());
    }
    if let Some(value_type) = context.literal_types.get(text) {
        return expected_or_exact(value_type.clone(), expected, span);
    }
    if text == "unit" {
        return expected_or_exact(
            CheckedExpressionType::semantic(conduit_core::UNIT_INFO_ID),
            expected,
            span,
        );
    }
    if matches!(text, "true" | "false") {
        return expected_or_exact(boolean(), expected, span);
    }
    if crate::text_value::parse_quoted_text(text).is_some() {
        // A contextual native text literal retains its exact nominal identity.
        // Expansion validates the closed constant's bounds and laws before Play.
        if let Some(expected) = expected {
            if expected.value_kind().and_then(|kind| context.structured_types.get(kind)).is_some_and(|ty| matches!(ty.shape(), StructuredInfoTypeShape::Nominal { representation, .. } if matches!(representation.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == conduit_core::TEXT_INFO_ID))) {
                return Ok(expected.clone());
            }
        }
        return expected_or_exact(
            CheckedExpressionType::semantic("value/text"),
            expected,
            span,
        );
    }
    if expected
        .and_then(CheckedExpressionType::value_kind)
        .is_some_and(|kind| kind.as_str() == conduit_core::EXACT_DECIMAL_QUANTITY_INFO_ID)
    {
        conduit_core::ExactDecimalQuantity::parse_plot_literal(text).map_err(|refusal| {
            diagnostic(
                span,
                &format!("selected exact quantity profile refused '{text}': {refusal:?}"),
            )
        })?;
        return Ok(expected.unwrap().clone());
    }
    match conduit_core::Quantity::parse_plot_literal(text) {
        Ok(quantity) => {
            return expected_or_exact(
                CheckedExpressionType::semantic(quantity.dimension().info_id()),
                expected,
                span,
            )
        }
        Err(
            refusal @ (conduit_core::QuantityLiteralRefusal::RepresentationIneligible { .. }
            | conduit_core::QuantityLiteralRefusal::AmbiguousUnit),
        ) => {
            return refuse(
                span,
                &format!("quantity literal '{text}' refused: {refusal:?}"),
            )
        }
        Err(_) => {}
    }
    let Some(expected) = expected else {
        return refuse(
            span,
            "numeric literal needs an exact semantic type from its expression context",
        );
    };
    let Some(kind) = expected.value_kind() else {
        return refuse(span, "literal cannot inhabit this structural type");
    };
    let represented_kind = context
        .structured_types
        .get(kind)
        .and_then(|value_type| match value_type.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                match representation.shape() {
                    StructuredInfoTypeShape::Leaf(kind) => Some(kind.as_str()),
                    _ => None,
                }
            }
            _ => None,
        })
        .unwrap_or(kind.as_str());
    if represented_kind == conduit_core::F32_INFO_ID {
        crate::ieee_literal::f32_bits(text).map_err(|message| diagnostic(span, &message))?;
        return Ok(expected.clone());
    }
    if crate::integer_literal::canonicalize(text, represented_kind)
        .map_err(|message| diagnostic(span, &message))?
        .is_some()
        || matches!(represented_kind, "value/count" | "value/scalar")
    {
        return Ok(expected.clone());
    }
    refuse(span, "literal is incompatible with its exact expected type")
}

pub(super) fn qualified_variant(
    value: &ExpressionSyntax,
    member: &ExpressionProjection,
    span: Span,
    context: &ExpressionTypeContext<'_>,
) -> Option<Result<CheckedExpressionType, ExpressionTypeDiagnostic>> {
    let (ExpressionSyntax::Atomic(name), ExpressionProjection::Field(case)) = (value, member)
    else {
        return None;
    };
    if context.immutable_values.contains_key(&name.text) {
        return None;
    }
    let candidates = context.structured_types.iter().filter(|(_, value_type)| {
        matches!(value_type.shape(), StructuredInfoTypeShape::Variant { schema, .. }
            if schema.as_str().strip_prefix("type/").and_then(|text| text.rsplit_once('@')).map(|(name, _)| name) == Some(name.text.as_str()))
    }).collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() != 1 {
        return Some(refuse(span, "variant type name is ambiguous"));
    }
    let (identity, value_type) = candidates[0];
    let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        unreachable!()
    };
    Some(
        match cases.iter().find(|candidate| candidate.tag() == case.text) {
            Some(candidate) if matches!(candidate.payload_type().shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == conduit_core::UNIT_INFO_ID) => {
                Ok(CheckedExpressionType::Semantic(identity.clone()))
            }
            Some(_) => refuse(span, "payload-bearing variant requires its exact payload"),
            None => refuse(span, "variant type has no such case"),
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn variant(
    tag: &crate::SpannedText,
    payload: &ExpressionSyntax,
    span: Span,
    expected: Option<&CheckedExpressionType>,
    context: &ExpressionTypeContext<'_>,
    node_types: &mut Vec<CheckedExpressionNodeType>,
) -> Result<CheckedExpressionType, ExpressionTypeDiagnostic> {
    let qualified = tag.text.split_once('.');
    let (kind, value_type) = if let Some((owner, _)) = qualified {
        let mut candidates = context.structured_types.iter().filter(|(_, value_type)| {
            matches!(value_type.shape(), StructuredInfoTypeShape::Variant { schema, .. }
                if schema.as_str().strip_prefix("type/").and_then(|text| text.rsplit_once('@')).map(|(name, _)| name) == Some(owner))
        });
        let Some(candidate) = candidates.next() else {
            return refuse(span, "unknown exact variant type");
        };
        if candidates.next().is_some() {
            return refuse(span, "variant type name is ambiguous");
        }
        candidate
    } else {
        let Some(CheckedExpressionType::Semantic(kind)) = expected else {
            return refuse(
                span,
                "variant expressions require an exact expected variant type",
            );
        };
        let Some(value_type) = context.structured_types.get(kind) else {
            return refuse(span, "expected type is not a variant");
        };
        (kind, value_type)
    };
    let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        return refuse(span, "expected type is not a variant");
    };
    let case_name = qualified.map_or(tag.text.as_str(), |(_, case)| case);
    let Some(case) = cases.iter().find(|case| case.tag() == case_name) else {
        return refuse(span, "variant type has no such case");
    };
    let payload_type = structures::member(case.payload_type());
    let actual = infer(payload, Some(&payload_type), context, node_types)?;
    require(
        actual,
        &payload_type,
        span,
        "variant payload has the wrong exact type",
    )?;
    Ok(CheckedExpressionType::Semantic(kind.clone()))
}
