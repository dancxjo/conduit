//! Classify callable Kinds separately from intrinsic expression syntax.
use crate::CanonicalExpansionDiagnostic;
use alloc::boxed::Box;

pub(super) fn is_semantic_call(expression: &crate::ExpressionSyntax) -> bool {
    matches!(expression, crate::ExpressionSyntax::SemanticCall { kind, .. }
        if !crate::expression_semantic_call::is_intrinsic(&kind.text))
}

/// Replaces the one dynamic semantic result in an outer pure expression with
/// that expression's ordinary input. More than one call, or another use of the
/// incoming value outside the call, would require synchronizing independent
/// runtime values and therefore has no implicit lowering here.
pub(super) fn isolate_nested_semantic_call(
    expression: &crate::ExpressionSyntax,
) -> Result<(crate::ExpressionSyntax, crate::ExpressionSyntax), CanonicalExpansionDiagnostic> {
    fn rewrite(
        expression: &crate::ExpressionSyntax,
        call: &mut Option<crate::ExpressionSyntax>,
    ) -> Result<crate::ExpressionSyntax, CanonicalExpansionDiagnostic> {
        use crate::ExpressionSyntax;
        Ok(match expression {
            ExpressionSyntax::SemanticCall {
                kind,
                arguments,
                span,
            } if crate::expression_semantic_call::is_intrinsic(&kind.text) => {
                ExpressionSyntax::SemanticCall {
                    kind: kind.clone(),
                    arguments: arguments
                        .iter()
                        .map(|argument| rewrite(argument, call))
                        .collect::<Result<_, _>>()?,
                    span: *span,
                }
            }
            ExpressionSyntax::SemanticCall { span, .. } => {
                if call.replace(expression.clone()).is_some() {
                    return Err(CanonicalExpansionDiagnostic::new(
                        "CND-FRM-046",
                        "one pure expression cannot implicitly synchronize multiple semantic call results; route them through an explicit temporal Gear such as flow/zip, state/combine-latest, or flow/join/by-key before applying arithmetic"
                            .into(),
                    ));
                }
                ExpressionSyntax::Input(*span)
            }
            ExpressionSyntax::Input(_) => {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    "an expression around a semantic call may depend only on that call result and constants; use current/sample when an event samples Current, or an explicit flow/zip, state/combine-latest, or flow/join/by-key Gear for two independent runtime values"
                        .into(),
                ));
            }
            ExpressionSyntax::Atomic(_) | ExpressionSyntax::TypedGlyphLiteral(_) => {
                expression.clone()
            }
            ExpressionSyntax::Projection {
                value,
                member,
                span,
            } => ExpressionSyntax::Projection {
                value: Box::new(rewrite(value, call)?),
                member: member.clone(),
                span: *span,
            },
            ExpressionSyntax::Unary {
                operator,
                operand,
                span,
            } => ExpressionSyntax::Unary {
                operator: *operator,
                operand: Box::new(rewrite(operand, call)?),
                span: *span,
            },
            ExpressionSyntax::Binary {
                operator,
                left,
                right,
                span,
            } => ExpressionSyntax::Binary {
                operator: *operator,
                left: Box::new(rewrite(left, call)?),
                right: Box::new(rewrite(right, call)?),
                span: *span,
            },
            ExpressionSyntax::Conditional {
                condition,
                when_true,
                when_false,
                span,
            } => ExpressionSyntax::Conditional {
                condition: Box::new(rewrite(condition, call)?),
                when_true: Box::new(rewrite(when_true, call)?),
                when_false: Box::new(rewrite(when_false, call)?),
                span: *span,
            },
            ExpressionSyntax::Tuple { values, span } => ExpressionSyntax::Tuple {
                values: values
                    .iter()
                    .map(|value| rewrite(value, call))
                    .collect::<Result<_, _>>()?,
                span: *span,
            },
            ExpressionSyntax::Collection { values, span } => ExpressionSyntax::Collection {
                values: values
                    .iter()
                    .map(|value| rewrite(value, call))
                    .collect::<Result<_, _>>()?,
                span: *span,
            },
            ExpressionSyntax::Record { fields, span } => ExpressionSyntax::Record {
                fields: fields
                    .iter()
                    .map(|field| {
                        let mut field = field.clone();
                        field.value = rewrite(&field.value, call)?;
                        Ok(field)
                    })
                    .collect::<Result<_, CanonicalExpansionDiagnostic>>()?,
                span: *span,
            },
            ExpressionSyntax::Variant { tag, payload, span } => ExpressionSyntax::Variant {
                tag: tag.clone(),
                payload: Box::new(rewrite(payload, call)?),
                span: *span,
            },
        })
    }

    let mut call = None;
    let outer = rewrite(expression, &mut call)?;
    Ok((
        outer,
        call.expect("caller established that a semantic call is present"),
    ))
}

pub(super) fn contains_semantic_call(expression: &crate::ExpressionSyntax) -> bool {
    match expression {
        crate::ExpressionSyntax::SemanticCall { arguments, .. } => {
            is_semantic_call(expression) || arguments.iter().any(contains_semantic_call)
        }
        crate::ExpressionSyntax::Projection { value, .. }
        | crate::ExpressionSyntax::Unary { operand: value, .. } => contains_semantic_call(value),
        crate::ExpressionSyntax::Binary { left, right, .. } => {
            contains_semantic_call(left) || contains_semantic_call(right)
        }
        crate::ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            contains_semantic_call(condition)
                || contains_semantic_call(when_true)
                || contains_semantic_call(when_false)
        }
        crate::ExpressionSyntax::Tuple { values, .. }
        | crate::ExpressionSyntax::Collection { values, .. } => {
            values.iter().any(contains_semantic_call)
        }
        crate::ExpressionSyntax::Record { fields, .. } => fields
            .iter()
            .any(|field| contains_semantic_call(&field.value)),
        crate::ExpressionSyntax::Variant { payload, .. } => contains_semantic_call(payload),
        crate::ExpressionSyntax::Input(_)
        | crate::ExpressionSyntax::Atomic(_)
        | crate::ExpressionSyntax::TypedGlyphLiteral(_) => false,
    }
}
