use super::*;
use crate::ExpressionSyntax;

pub(super) fn substitute_immutable_values(
    expression: &ExpressionSyntax,
    source_plot: &CheckedCanonicalPlot,
    environment: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<ExpressionSyntax, CanonicalExpansionDiagnostic> {
    let substitute = |value: &crate::SpannedText| {
        let local = source_plot
            .local_values
            .iter()
            .find(|(name, _)| name == &value.text)
            .map(|(_, local)| local)
            .or_else(|| {
                source_plot
                    .startup_parameters
                    .iter()
                    .any(|parameter| parameter.name == value.text)
                    .then(|| environment.get(&value.text))
                    .flatten()
            });
        let Some(local) = local else {
            return Ok(None);
        };
        let local = match local {
            CanonicalStartupValue::PlotParameter(name) => {
                environment.get(name).ok_or_else(|| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-046",
                        format!("immutable local '{}' has no bound value", value.text),
                    )
                })?
            }
            local => local,
        };
        if matches!(local, CanonicalStartupValue::Structured(_)) {
            return Ok(None);
        }
        let text = match local {
            CanonicalStartupValue::Literal(text) => text.clone(),
            CanonicalStartupValue::Quantity(_)
            | CanonicalStartupValue::Unit(_)
            | CanonicalStartupValue::TemperatureDifference(_) => return Ok(None),
            _ => {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    format!(
                        "immutable local '{}' has no canonical pure-expression value",
                        value.text
                    ),
                ));
            }
        };
        Ok(Some(ExpressionSyntax::Atomic(crate::SpannedText {
            text,
            span: value.span,
        })))
    };

    Ok(match expression {
        ExpressionSyntax::Atomic(value) => substitute(value)?.unwrap_or_else(|| expression.clone()),
        ExpressionSyntax::Input(_) | ExpressionSyntax::TypedGlyphLiteral(_) => expression.clone(),
        ExpressionSyntax::Projection {
            value,
            member,
            span,
        } => ExpressionSyntax::Projection {
            value: Box::new(substitute_immutable_values(
                value,
                source_plot,
                environment,
            )?),
            member: member.clone(),
            span: *span,
        },
        ExpressionSyntax::Unary {
            operator,
            operand,
            span,
        } => ExpressionSyntax::Unary {
            operator: *operator,
            operand: Box::new(substitute_immutable_values(
                operand,
                source_plot,
                environment,
            )?),
            span: *span,
        },
        ExpressionSyntax::Binary {
            operator,
            left,
            right,
            span,
        } => ExpressionSyntax::Binary {
            operator: *operator,
            left: Box::new(substitute_immutable_values(left, source_plot, environment)?),
            right: Box::new(substitute_immutable_values(
                right,
                source_plot,
                environment,
            )?),
            span: *span,
        },
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            span,
        } => ExpressionSyntax::Conditional {
            condition: Box::new(substitute_immutable_values(
                condition,
                source_plot,
                environment,
            )?),
            when_true: Box::new(substitute_immutable_values(
                when_true,
                source_plot,
                environment,
            )?),
            when_false: Box::new(substitute_immutable_values(
                when_false,
                source_plot,
                environment,
            )?),
            span: *span,
        },
        ExpressionSyntax::Tuple { values, span } => ExpressionSyntax::Tuple {
            values: substitute_values(values, source_plot, environment)?,
            span: *span,
        },
        ExpressionSyntax::Collection { values, span } => ExpressionSyntax::Collection {
            values: substitute_values(values, source_plot, environment)?,
            span: *span,
        },
        ExpressionSyntax::Record { fields, span } => ExpressionSyntax::Record {
            fields: fields
                .iter()
                .map(|field| {
                    Ok(crate::StructuredExpressionField {
                        name: field.name.clone(),
                        value: substitute_immutable_values(&field.value, source_plot, environment)?,
                        punned: field.punned,
                        span: field.span,
                    })
                })
                .collect::<Result<_, CanonicalExpansionDiagnostic>>()?,
            span: *span,
        },
        ExpressionSyntax::Variant { tag, payload, span } => ExpressionSyntax::Variant {
            tag: tag.clone(),
            payload: Box::new(substitute_immutable_values(
                payload,
                source_plot,
                environment,
            )?),
            span: *span,
        },
        ExpressionSyntax::SemanticCall {
            kind,
            arguments,
            span,
        } => ExpressionSyntax::SemanticCall {
            kind: kind.clone(),
            arguments: substitute_values(arguments, source_plot, environment)?,
            span: *span,
        },
    })
}

fn substitute_values(
    values: &[ExpressionSyntax],
    source_plot: &CheckedCanonicalPlot,
    environment: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<Vec<ExpressionSyntax>, CanonicalExpansionDiagnostic> {
    values
        .iter()
        .map(|value| substitute_immutable_values(value, source_plot, environment))
        .collect()
}

/// Retain concrete checked Info without inventing an authored syntax node.
pub(super) fn structured_constants(
    expression: &ExpressionSyntax,
    source_plot: &CheckedCanonicalPlot,
    environment: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<BTreeMap<String, conduit_core::StructuredInfoValue>, CanonicalExpansionDiagnostic> {
    let mut names = BTreeSet::new();
    immutable_names(expression, &mut names);
    let mut values = BTreeMap::new();
    for name in names {
        let local = source_plot
            .local_values
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
            .or_else(|| {
                source_plot
                    .startup_parameters
                    .iter()
                    .any(|parameter| parameter.name == name)
                    .then(|| environment.get(name))
                    .flatten()
            });
        let Some(local) = local else { continue };
        let resolved = super::super::startup::substitute(local, environment)?;
        if let CanonicalStartupValue::Structured(value) = resolved {
            let concrete = value.try_concrete().ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    format!("immutable structured value '{name}' remains unresolved"),
                )
            })?;
            values.insert(name.into(), concrete);
        }
    }
    Ok(values)
}

fn immutable_names<'a>(expression: &'a ExpressionSyntax, names: &mut BTreeSet<&'a str>) {
    match expression {
        ExpressionSyntax::Atomic(value) => {
            names.insert(&value.text);
        }
        ExpressionSyntax::Input(_) | ExpressionSyntax::TypedGlyphLiteral(_) => (),
        ExpressionSyntax::Projection { value, .. } => immutable_names(value, names),
        ExpressionSyntax::Unary { operand, .. } => immutable_names(operand, names),
        ExpressionSyntax::Binary { left, right, .. } => {
            immutable_names(left, names);
            immutable_names(right, names);
        }
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            immutable_names(condition, names);
            immutable_names(when_true, names);
            immutable_names(when_false, names);
        }
        ExpressionSyntax::Tuple { values, .. }
        | ExpressionSyntax::Collection { values, .. }
        | ExpressionSyntax::SemanticCall {
            arguments: values, ..
        } => {
            for value in values {
                immutable_names(value, names);
            }
        }
        ExpressionSyntax::Record { fields, .. } => {
            for field in fields {
                immutable_names(&field.value, names);
            }
        }
        ExpressionSyntax::Variant { payload, .. } => immutable_names(payload, names),
    }
}
