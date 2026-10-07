use super::*;
use crate::ExpressionSyntax;

pub(super) fn substitute_immutable_values(
    expression: &ExpressionSyntax,
    source_plot: &CheckedCanonicalPlot,
    environment: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<ExpressionSyntax, CanonicalExpansionDiagnostic> {
    let mut result = substitute_values_inner(expression, source_plot, environment, true)?;
    // Captured structures insert multiple nodes at one original reference. Spans
    // key the internal node-type table, so assign bounded unique bookkeeping
    // offsets while retaining every original diagnostic line and column.
    if environment
        .values()
        .chain(source_plot.local_values.iter().map(|(_, value)| value))
        .any(|value| matches!(value, CanonicalStartupValue::Structured(_)))
    {
        normalize_spans(&mut result, &mut 0)?;
    }
    Ok(result)
}

pub(super) fn retained_capture_expression(
    expression: &ExpressionSyntax,
    source_plot: &CheckedCanonicalPlot,
    environment: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<
    (
        ExpressionSyntax,
        BTreeMap<String, crate::CheckedExpressionType>,
    ),
    CanonicalExpansionDiagnostic,
> {
    let mut types = BTreeMap::new();
    for (name, value) in environment.iter().chain(
        source_plot
            .local_values
            .iter()
            .map(|(name, value)| (name, value)),
    ) {
        if let CanonicalStartupValue::Structured(value) = value {
            let profile = value.value_type().profile().map_err(|_| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    format!("captured startup '{name}' has no bounded exact profile"),
                )
            })?;
            types.insert(
                name.clone(),
                crate::CheckedExpressionType::Semantic(profile.value_kind().clone()),
            );
        }
    }
    Ok((
        substitute_values_inner(expression, source_plot, environment, false)?,
        types,
    ))
}

fn normalize_spans(
    expression: &mut ExpressionSyntax,
    counter: &mut usize,
) -> Result<(), CanonicalExpansionDiagnostic> {
    let span = match expression {
        ExpressionSyntax::Atomic(value) => &mut value.span,
        ExpressionSyntax::Input(span) => span,
        ExpressionSyntax::Projection { value, span, .. } => {
            normalize_spans(value, counter)?;
            span
        }
        ExpressionSyntax::Unary { operand, span, .. } => {
            normalize_spans(operand, counter)?;
            span
        }
        ExpressionSyntax::Binary {
            left, right, span, ..
        } => {
            normalize_spans(left, counter)?;
            normalize_spans(right, counter)?;
            span
        }
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            span,
        } => {
            normalize_spans(condition, counter)?;
            normalize_spans(when_true, counter)?;
            normalize_spans(when_false, counter)?;
            span
        }
        ExpressionSyntax::Tuple { values, span }
        | ExpressionSyntax::Collection { values, span } => {
            for value in values {
                normalize_spans(value, counter)?;
            }
            span
        }
        ExpressionSyntax::Record { fields, span } => {
            for field in fields {
                normalize_spans(&mut field.value, counter)?;
            }
            span
        }
        ExpressionSyntax::Variant { payload, span, .. } => {
            normalize_spans(payload, counter)?;
            span
        }
        ExpressionSyntax::SemanticCall {
            arguments, span, ..
        } => {
            for argument in arguments {
                normalize_spans(argument, counter)?;
            }
            span
        }
    };
    if *counter >= conduit_core::MAXIMUM_STRUCTURED_INFO_NODES {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            "captured expression exceeds bounded node count".into(),
        ));
    }
    span.start = *counter;
    *counter = counter.checked_add(1).ok_or_else(|| {
        CanonicalExpansionDiagnostic::new("CND-FRM-046", "captured expression span overflow".into())
    })?;
    span.end = *counter;
    Ok(())
}

fn captured_value<'a>(
    expression: &ExpressionSyntax,
    source_plot: &'a CheckedCanonicalPlot,
    environment: &'a BTreeMap<String, CanonicalStartupValue>,
) -> Option<&'a crate::CanonicalStructuredStartupValue> {
    let mut root = expression;
    let mut members = Vec::new();
    while let ExpressionSyntax::Projection { value, member, .. } = root {
        members.push(member);
        root = value;
    }
    let ExpressionSyntax::Atomic(name) = root else {
        return None;
    };
    let mut selected = source_plot
        .local_values
        .iter()
        .find(|(local, _)| local == &name.text)
        .map(|(_, value)| value)
        .or_else(|| environment.get(&name.text))?;
    if let CanonicalStartupValue::PlotParameter(name) = selected {
        selected = environment.get(name)?;
    }
    let CanonicalStartupValue::Structured(selected_value) = selected else {
        return None;
    };
    let mut value = selected_value;
    for member in members.iter().rev() {
        value = value.projected(member)?;
    }
    Some(value)
}

fn substitute_values_inner(
    expression: &ExpressionSyntax,
    source_plot: &CheckedCanonicalPlot,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    reify_structured: bool,
) -> Result<ExpressionSyntax, CanonicalExpansionDiagnostic> {
    if reify_structured {
        if let ExpressionSyntax::SemanticCall {
            kind,
            arguments,
            span,
        } = expression
        {
            if kind.text == "variant/is" && arguments.len() == 2 {
                if let Some(value) = captured_value(&arguments[0], source_plot, environment) {
                    if let ExpressionSyntax::Atomic(case) = &arguments[1] {
                        if let (Some(tag), Some(case)) = (
                            value.variant_tag(),
                            crate::text_value::parse_quoted_text(&case.text),
                        ) {
                            return Ok(ExpressionSyntax::Atomic(crate::SpannedText {
                                text: (tag == case).to_string(),
                                span: *span,
                            }));
                        }
                    }
                }
            }
        }
    }
    if reify_structured {
        if let Some(value) = captured_value(expression, source_plot, environment) {
            return value.expression_syntax(expression.span()).ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    "captured startup has no supported concrete projection".into(),
                )
            });
        }
    }
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
        if let CanonicalStartupValue::Structured(structured) = local {
            if !reify_structured {
                return Ok(None);
            }
            return structured
                .expression_syntax(value.span)
                .map(Some)
                .ok_or_else(|| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-046",
                        format!(
                            "immutable structured value '{}' has no supported exact expression",
                            value.text
                        ),
                    )
                });
        }
        let text = match local {
            CanonicalStartupValue::Literal(text) => text.clone(),
            CanonicalStartupValue::Quantity(quantity) => {
                format!("{}{}", quantity.value(), quantity.unit().plot_suffix())
            }
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
        ExpressionSyntax::Input(_) => expression.clone(),
        ExpressionSyntax::Projection {
            value,
            member,
            span,
        } => ExpressionSyntax::Projection {
            value: Box::new(substitute_values_inner(
                value,
                source_plot,
                environment,
                reify_structured,
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
            operand: Box::new(substitute_values_inner(
                operand,
                source_plot,
                environment,
                reify_structured,
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
            left: Box::new(substitute_values_inner(
                left,
                source_plot,
                environment,
                reify_structured,
            )?),
            right: Box::new(substitute_values_inner(
                right,
                source_plot,
                environment,
                reify_structured,
            )?),
            span: *span,
        },
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            span,
        } => ExpressionSyntax::Conditional {
            condition: Box::new(substitute_values_inner(
                condition,
                source_plot,
                environment,
                reify_structured,
            )?),
            when_true: Box::new(substitute_values_inner(
                when_true,
                source_plot,
                environment,
                reify_structured,
            )?),
            when_false: Box::new(substitute_values_inner(
                when_false,
                source_plot,
                environment,
                reify_structured,
            )?),
            span: *span,
        },
        ExpressionSyntax::Tuple { values, span } => ExpressionSyntax::Tuple {
            values: substitute_values(values, source_plot, environment, reify_structured)?,
            span: *span,
        },
        ExpressionSyntax::Collection { values, span } => ExpressionSyntax::Collection {
            values: substitute_values(values, source_plot, environment, reify_structured)?,
            span: *span,
        },
        ExpressionSyntax::Record { fields, span } => ExpressionSyntax::Record {
            fields: fields
                .iter()
                .map(|field| {
                    Ok(crate::StructuredExpressionField {
                        name: field.name.clone(),
                        value: substitute_values_inner(
                            &field.value,
                            source_plot,
                            environment,
                            reify_structured,
                        )?,
                        punned: field.punned,
                        span: field.span,
                    })
                })
                .collect::<Result<_, CanonicalExpansionDiagnostic>>()?,
            span: *span,
        },
        ExpressionSyntax::Variant { tag, payload, span } => ExpressionSyntax::Variant {
            tag: tag.clone(),
            payload: Box::new(substitute_values_inner(
                payload,
                source_plot,
                environment,
                reify_structured,
            )?),
            span: *span,
        },
        ExpressionSyntax::SemanticCall {
            kind,
            arguments,
            span,
        } => ExpressionSyntax::SemanticCall {
            kind: kind.clone(),
            arguments: substitute_values(arguments, source_plot, environment, reify_structured)?,
            span: *span,
        },
    })
}

fn substitute_values(
    values: &[ExpressionSyntax],
    source_plot: &CheckedCanonicalPlot,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    reify_structured: bool,
) -> Result<Vec<ExpressionSyntax>, CanonicalExpansionDiagnostic> {
    values
        .iter()
        .map(|value| substitute_values_inner(value, source_plot, environment, reify_structured))
        .collect()
}
