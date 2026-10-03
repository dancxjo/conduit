use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn expand_semantic_call_graph(
    expression: &crate::ExpressionSyntax,
    input_kind: &conduit_core::KindId,
    expected_kind: Option<&conduit_core::KindId>,
    source_span: crate::Span,
    source_plot: &CheckedCanonicalPlot,
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    catalog: &ProfileCatalog,
    path: &[String],
    gears: &mut Vec<CheckedGear>,
    connections: &mut Vec<CheckedConnection>,
    provenance: &mut Vec<ExpandedGearProvenance>,
    gear_ids: &mut BTreeSet<GearId>,
    anonymous_counts: &mut BTreeMap<String, usize>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    if !matches!(expression, crate::ExpressionSyntax::SemanticCall { .. }) {
        let expected_kind = expected_kind.ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "nested semantic expression has no exact output Kind".into(),
            )
        })?;
        return expand_argument_expression(
            expression,
            input_kind,
            expected_kind,
            source_span,
            source_plot,
            structured_types,
            catalog,
            path,
            gears,
            connections,
            provenance,
            gear_ids,
            anonymous_counts,
        );
    }
    let crate::ExpressionSyntax::SemanticCall {
        kind, arguments, ..
    } = expression
    else {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            "semantic call graph must be rooted at a semantic Kind call".into(),
        ));
    };
    if arguments.is_empty() {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            "zero-input semantic calls need an explicit activation law before they can appear in a one-input expression"
                .into(),
        ));
    }
    let root = expand_one(
        kind,
        source_span,
        source_plot,
        catalog,
        path,
        gears,
        provenance,
        gear_ids,
        anonymous_counts,
    )?;
    let sinks = root
        .input
        .clone()
        .expect("checked semantic call has at least one input");
    let mut exposed_inputs = Vec::new();
    for (argument, sink) in arguments.iter().zip(sinks) {
        let StageSink::Internal(sink) = sink else {
            unreachable!("semantic call inputs are internal ports")
        };
        if matches!(argument, crate::ExpressionSyntax::Input(_)) {
            exposed_inputs.push(StageSink::Internal(sink));
            continue;
        }
        let argument_stage = if matches!(argument, crate::ExpressionSyntax::SemanticCall { .. }) {
            expand_semantic_call_graph(
                argument,
                input_kind,
                Some(&sink.port.value_kind),
                source_span,
                source_plot,
                structured_types,
                catalog,
                path,
                gears,
                connections,
                provenance,
                gear_ids,
                anonymous_counts,
            )?
        } else {
            expand_argument_expression(
                argument,
                input_kind,
                &sink.port.value_kind,
                source_span,
                source_plot,
                structured_types,
                catalog,
                path,
                gears,
                connections,
                provenance,
                gear_ids,
                anonymous_counts,
            )?
        };
        let StageSource::Internal(source) = argument_stage
            .output
            .expect("semantic call argument has one output")
        else {
            unreachable!("semantic call argument output is internal")
        };
        connect(source, sink, connections);
        exposed_inputs.extend(argument_stage.input.unwrap_or_default());
    }
    Ok(Stage {
        input: Some(exposed_inputs),
        output: root.output,
    })
}

#[allow(clippy::too_many_arguments)]
fn expand_argument_expression(
    expression: &crate::ExpressionSyntax,
    input_kind: &conduit_core::KindId,
    expected_kind: &conduit_core::KindId,
    source_span: crate::Span,
    source_plot: &CheckedCanonicalPlot,
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    catalog: &ProfileCatalog,
    path: &[String],
    gears: &mut Vec<CheckedGear>,
    connections: &mut Vec<CheckedConnection>,
    provenance: &mut Vec<ExpandedGearProvenance>,
    gear_ids: &mut BTreeSet<GearId>,
    anonymous_counts: &mut BTreeMap<String, usize>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    if contains_semantic_call(expression) {
        let (outer_expression, nested_call) = isolate_nested_semantic_call(expression)?;
        let nested = expand_semantic_call_graph(
            &nested_call,
            input_kind,
            None,
            source_span,
            source_plot,
            structured_types,
            catalog,
            path,
            gears,
            connections,
            provenance,
            gear_ids,
            anonymous_counts,
        )?;
        let StageSource::Internal(nested_output) = nested
            .output
            .clone()
            .expect("nested semantic call has one output")
        else {
            unreachable!("nested semantic call output is internal")
        };
        let outer = expand_argument_expression(
            &outer_expression,
            &nested_output.endpoint.port.value_kind,
            expected_kind,
            source_span,
            source_plot,
            structured_types,
            catalog,
            path,
            gears,
            connections,
            provenance,
            gear_ids,
            anonymous_counts,
        )?;
        let [StageSink::Internal(outer_input)] = outer
            .input
            .as_deref()
            .expect("pure expression has one input")
        else {
            unreachable!("pure expression has exactly one internal input")
        };
        connect(nested_output, outer_input.clone(), connections);
        return Ok(Stage {
            input: nested.input,
            output: outer.output,
        });
    }
    let input_type = crate::CheckedExpressionType::Semantic(input_kind.clone());
    let expected_type = crate::CheckedExpressionType::Semantic(expected_kind.clone());
    let semantic_kinds = catalog
        .canonical_kinds()
        .values()
        .cloned()
        .map(|kind| (kind.kind_id.as_str().to_string(), kind))
        .collect::<BTreeMap<_, _>>();
    let checked = crate::expression_check::check_expression_as(
        expression,
        Some(&expected_type),
        &crate::ExpressionTypeContext {
            input: &input_type,
            immutable_values: &BTreeMap::new(),
            structured_types,
            literal_types: &BTreeMap::new(),
            numeric_types: &BTreeSet::new(),
            semantic_kinds: &semantic_kinds,
        },
    )
    .map_err(|diagnostic| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            format!(
                "semantic call argument at {}:{} is not well typed: {}",
                diagnostic.span.line, diagnostic.span.column, diagnostic.message
            ),
        )
    })?;
    let definition =
        crate::pure_expression_definition(&checked, PortTemporal::Value).map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "semantic call argument has no finite exact Port identity".into(),
            )
        })?;
    let key = definition.kind_id.as_str().to_string();
    let count = anonymous_counts.entry(key.clone()).or_default();
    let name = format!("expression-{}-{count}", &hash_string(&key)[..12]);
    *count += 1;
    let mut child_path = path.to_vec();
    child_path.push(name.clone());
    let gear_id = GearId::from(child_path.join("/"));
    if !gear_ids.insert(gear_id.clone()) {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-038",
            format!("expanded gear path '{}' is not unique", gear_id.as_str()),
        ));
    }
    let input = definition.inputs[0].clone();
    let output = definition.outputs[0].clone();
    gears.push(crate::checked_gear_from_parts! {
        gear_id: gear_id.clone(),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        startup_parameters: vec![conduit_core::FrontStartupParameter {
            name: "program".into(),
            value_type: conduit_core::kind_id("value/text"),
            has_default: false,
        }],
        shorthand: Some((input.port_id.clone(), output.port_id.clone())),
        inputs: vec![input.clone()],
        outputs: vec![output.clone()],
        semantic_contract: conduit_core::KindSemanticContract {
            configuration: definition.configuration.clone(),
            laws: crate::pure_expression_semantic_laws(),
        },
        terminal_transductions: Vec::new(),
        resource_ports: Vec::new(),
        configuration: definition
            .configuration
            .into_iter()
            .map(|field| conduit_core::ConfigurationEntry {
                key: field.key,
                value: field.default_value,
            })
            .collect(),
        pool_references: Vec::new(),
    });
    provenance.push(ExpandedGearProvenance {
        gear_id: gear_id.as_str().to_string(),
        plot_path: path.to_vec(),
        source_plot: source_plot.name.clone(),
        source_gear: name,
        source_span,
    });
    Ok(Stage {
        input: Some(vec![StageSink::Internal(TrackedEndpoint::payload(
            Endpoint {
                gear_id: gear_id.clone(),
                port: input,
            },
        ))]),
        output: Some(StageSource::Internal(TrackedEndpoint::payload(Endpoint {
            gear_id,
            port: output,
        }))),
    })
}

/// Replaces the one dynamic semantic result in an outer pure expression with
/// that expression's ordinary input. More than one call, or another use of the
/// incoming value outside the call, would require synchronizing independent
/// runtime values and therefore has no implicit lowering here.
fn isolate_nested_semantic_call(
    expression: &crate::ExpressionSyntax,
) -> Result<(crate::ExpressionSyntax, crate::ExpressionSyntax), CanonicalExpansionDiagnostic> {
    fn rewrite(
        expression: &crate::ExpressionSyntax,
        call: &mut Option<crate::ExpressionSyntax>,
    ) -> Result<crate::ExpressionSyntax, CanonicalExpansionDiagnostic> {
        use crate::ExpressionSyntax;
        Ok(match expression {
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
            ExpressionSyntax::Atomic(_) => expression.clone(),
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
        crate::ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } => {
            !matches!(kind.text.as_str(), "variant/is" | "variant/tag")
                || arguments.iter().any(contains_semantic_call)
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
        crate::ExpressionSyntax::Input(_) | crate::ExpressionSyntax::Atomic(_) => false,
    }
}

fn connect(
    source: TrackedEndpoint,
    sink: TrackedEndpoint,
    connections: &mut Vec<CheckedConnection>,
) {
    let source_track = source.track;
    let Endpoint {
        gear_id: source_gear_id,
        port: source_port,
    } = source.endpoint;
    let Endpoint {
        gear_id: sink_gear_id,
        port: sink_port,
    } = sink.endpoint;
    connections.push(CheckedConnection {
        source_gear_id,
        source_port_id: source_port.port_id,
        sink_gear_id,
        sink_port_id: sink_port.port_id,
        value_kind: source_port.value_kind,
        track: source_track,
        temporal: source_port.temporal,
    });
}

#[allow(clippy::too_many_arguments)]
fn expand_one(
    kind_name: &crate::SpannedText,
    source_span: crate::Span,
    source_plot: &CheckedCanonicalPlot,
    catalog: &ProfileCatalog,
    path: &[String],
    gears: &mut Vec<CheckedGear>,
    provenance: &mut Vec<ExpandedGearProvenance>,
    gear_ids: &mut BTreeSet<GearId>,
    anonymous_counts: &mut BTreeMap<String, usize>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    let kind = catalog
        .canonical_kind(&conduit_core::kind_id(&kind_name.text))
        .ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                format!(
                    "pure semantic call '{}' has no canonical Kind",
                    kind_name.text
                ),
            )
        })?;
    let count = anonymous_counts
        .entry(format!("semantic-call:{}", kind.kind_id.as_str()))
        .or_default();
    let name = format!(
        "semantic-call-{}-{count}",
        kind.kind_id.as_str().replace('/', "-")
    );
    *count += 1;
    let mut child_path = path.to_vec();
    child_path.push(name.clone());
    let gear_id = GearId::from(child_path.join("/"));
    if !gear_ids.insert(gear_id.clone()) {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-038",
            format!("expanded gear path '{}' is not unique", gear_id.as_str()),
        ));
    }
    let output = kind.outputs[0].clone();
    gears.push(crate::checked_gear_from_parts! {
        gear_id: gear_id.clone(),
        kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        startup_parameters: kind.startup_parameters.clone(),
        shorthand: kind.shorthand.clone(),
        inputs: kind.inputs.clone(),
        outputs: kind.outputs.clone(),
        semantic_contract: kind.semantic_contract(),
        terminal_transductions: kind.terminal_transductions().cloned().collect(),
        resource_ports: kind.resource_ports().to_vec(),
        configuration: kind
            .configuration
            .iter()
            .map(|field| conduit_core::ConfigurationEntry {
                key: field.key.clone(),
                value: field.default_value.clone(),
            })
            .collect(),
        pool_references: Vec::new(),
    });
    provenance.push(ExpandedGearProvenance {
        gear_id: gear_id.as_str().to_string(),
        plot_path: path.to_vec(),
        source_plot: source_plot.name.clone(),
        source_gear: name,
        source_span,
    });
    Ok(Stage {
        input: Some(
            kind.inputs
                .iter()
                .cloned()
                .map(|port| {
                    StageSink::Internal(TrackedEndpoint::payload(Endpoint {
                        gear_id: gear_id.clone(),
                        port,
                    }))
                })
                .collect(),
        ),
        output: Some(StageSource::Internal(TrackedEndpoint::payload(Endpoint {
            gear_id,
            port: output,
        }))),
    })
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;

    fn expression(source: &str) -> crate::ExpressionSyntax {
        crate::pure_expression::parse(source, source, 0).unwrap()
    }

    #[test]
    fn ambiguous_runtime_arithmetic_names_the_explicit_temporal_families() {
        let error =
            isolate_nested_semantic_call(&expression("math/left(.) + math/right(.)")).unwrap_err();
        assert_eq!(error.code, "CND-FRM-046");
        for family in ["flow/zip", "state/combine-latest", "flow/join/by-key"] {
            assert!(error.message.contains(family));
        }

        let error = isolate_nested_semantic_call(&expression(". + math/derive(.)")).unwrap_err();
        assert!(error.message.contains("current/sample"));
        assert!(error.message.contains("two independent runtime values"));
    }
}
