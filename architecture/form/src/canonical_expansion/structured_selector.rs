use super::*;
use crate::{KindConfigurationField, KindConfigurationRule, KindProjection};
use conduit_core::{
    ConfigurationValue, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
    StructuredSelector, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

mod temporal;
use temporal::{input_temporal, output_temporal};

pub fn structured_selector_definition(
    selector: &StructuredSelector,
    temporal: PortTemporal,
) -> KindProjection {
    let kind_id = selector
        .kind_id(temporal)
        .expect("checked selector has a finite semantic identity");
    KindProjection {
        kind_id,
        kind_contract_revision: KindIdentity::from("structured-info/selector-operation@1"),
        inputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("input"),
            value_kind: selector
                .input_type()
                .profile()
                .expect("checked selector input has a finite profile")
                .value_kind()
                .clone(),
            direction: PortDirection::Input,
            temporal,
        }],
        outputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("output"),
            value_kind: selector
                .output_type()
                .profile()
                .expect("checked selector output has a finite profile")
                .value_kind()
                .clone(),
            direction: PortDirection::Output,
            temporal,
        }],
        configuration: vec![KindConfigurationField {
            key: "selector".to_string(),
            default_value: ConfigurationValue::Text(
                selector
                    .canonical_hex()
                    .expect("checked selector has a finite canonical configuration"),
            ),
            rule: KindConfigurationRule::TextBytes {
                maximum: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
            },
        }],
    }
}

pub(super) enum PendingStage {
    Ready(Stage),
    Expression {
        expression: crate::ExpressionSyntax,
        source_span: crate::Span,
    },
    Selector {
        selector: StructuredSelector,
        source_span: crate::Span,
    },
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_selectors(
    pending: Vec<PendingStage>,
    source_form: &CheckedCanonicalForm,
    forms: &BTreeMap<&str, &CheckedCanonicalForm>,
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::FormBack>,
    depth: usize,
    gears: &mut Vec<CheckedGear>,
    connections: &mut Vec<CheckedConnection>,
    shared_pools: &mut Vec<ExpandedSharedPool>,
    provenance: &mut Vec<ExpandedGearProvenance>,
    gear_ids: &mut BTreeSet<GearId>,
    anonymous_counts: &mut BTreeMap<String, usize>,
) -> Result<Vec<Stage>, CanonicalExpansionDiagnostic> {
    let mut stages = Vec::with_capacity(pending.len());
    for (index, stage) in pending.iter().enumerate() {
        let PendingStage::Selector {
            selector,
            source_span,
        } = stage
        else {
            match stage {
                PendingStage::Ready(stage) => stages.push(stage.clone()),
                PendingStage::Expression {
                    expression,
                    source_span,
                } => stages.push(expand_expression(
                    expression,
                    *source_span,
                    stages.last(),
                    &pending[index + 1..],
                    source_form,
                    structured_types,
                    catalog,
                    environment,
                    path,
                    gears,
                    provenance,
                    gear_ids,
                    anonymous_counts,
                )?),
                PendingStage::Selector { .. } => unreachable!(),
            }
            continue;
        };
        let left = pending[..index].iter().rev().find_map(|stage| match stage {
            PendingStage::Ready(stage) => output_temporal(stage),
            PendingStage::Selector { .. } | PendingStage::Expression { .. } => None,
        });
        let right = pending[index + 1..].iter().find_map(|stage| match stage {
            PendingStage::Ready(stage) => input_temporal(stage),
            PendingStage::Selector { .. } | PendingStage::Expression { .. } => None,
        });
        let temporal = match (left, right) {
            (
                Some(conduit_core::PortTemporal::Flow { .. }),
                Some(conduit_core::PortTemporal::Value),
            ) => left.expect("selector source temporal is present"),
            (Some(left), Some(right)) if left != right => {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-045",
                    "structured selector cannot change a Cord's temporal contract".into(),
                ));
            }
            (Some(temporal), _) | (_, Some(temporal)) => temporal,
            (None, None) => {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-036",
                    "structured selector requires an adjacent typed Cord stage".into(),
                ));
            }
        };
        let definition = structured_selector_definition(selector, temporal);
        let key = definition.kind_id.as_str().to_string();
        let count = anonymous_counts.entry(key.clone()).or_default();
        let name = format!("selector-{}-{count}", &hash_string(&key)[..12]);
        *count += 1;
        let selector_configuration = selector.canonical_hex().map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-045",
                "structured selector exceeds the finite canonical configuration bound".into(),
            )
        })?;
        let gear = CheckedCanonicalGear {
            name: None,
            kind: key,
            startup_parameters: vec![conduit_core::FrontStartupParameter {
                name: "selector".to_string(),
                value_type: conduit_core::kind_id("value/text"),
                has_default: false,
            }],
            startup_bindings: vec![crate::CheckedStartupBinding {
                name: "selector".to_string(),
                value_type: "Text".to_string(),
                value: CanonicalStartupValue::Literal(format!("\"{selector_configuration}\"")),
            }],
            retained: None,
            source_span: *source_span,
        };
        let instance = instantiate_gear(
            &gear,
            &name,
            source_form,
            forms,
            structured_types,
            catalog,
            backs,
            environment,
            path,
            stack,
            realization_backs,
            depth,
            gears,
            connections,
            shared_pools,
            provenance,
            gear_ids,
        )?;
        stages.push(stage_for_instance(&name, &instance, None)?);
    }
    Ok(stages)
}

fn substitute_immutable_values(
    expression: &crate::ExpressionSyntax,
    source_form: &CheckedCanonicalForm,
    environment: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<crate::ExpressionSyntax, CanonicalExpansionDiagnostic> {
    use crate::ExpressionSyntax;

    let substitute = |value: &crate::SpannedText| {
        let Some((_, local)) = source_form
            .local_values
            .iter()
            .find(|(name, _)| name == &value.text)
        else {
            return Ok(None);
        };
        let local = match local {
            CanonicalStartupValue::FormParameter(name) => {
                environment.get(name).ok_or_else(|| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-046",
                        format!("immutable local '{}' has no bound value", value.text),
                    )
                })?
            }
            local => local,
        };
        let text = match local {
            CanonicalStartupValue::Literal(text) => text.clone(),
            CanonicalStartupValue::Quantity(quantity) => {
                format!("{}{}", quantity.value(), quantity.unit().form_suffix())
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
            value: Box::new(substitute_immutable_values(
                value,
                source_form,
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
                source_form,
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
            left: Box::new(substitute_immutable_values(left, source_form, environment)?),
            right: Box::new(substitute_immutable_values(
                right,
                source_form,
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
                source_form,
                environment,
            )?),
            when_true: Box::new(substitute_immutable_values(
                when_true,
                source_form,
                environment,
            )?),
            when_false: Box::new(substitute_immutable_values(
                when_false,
                source_form,
                environment,
            )?),
            span: *span,
        },
        ExpressionSyntax::Tuple { values, span } => ExpressionSyntax::Tuple {
            values: values
                .iter()
                .map(|value| substitute_immutable_values(value, source_form, environment))
                .collect::<Result<_, _>>()?,
            span: *span,
        },
        ExpressionSyntax::Collection { values, span } => ExpressionSyntax::Collection {
            values: values
                .iter()
                .map(|value| substitute_immutable_values(value, source_form, environment))
                .collect::<Result<_, _>>()?,
            span: *span,
        },
        ExpressionSyntax::Record { fields, span } => ExpressionSyntax::Record {
            fields: fields
                .iter()
                .map(|field| {
                    Ok(crate::StructuredExpressionField {
                        name: field.name.clone(),
                        value: substitute_immutable_values(&field.value, source_form, environment)?,
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
                source_form,
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
            arguments: arguments
                .iter()
                .map(|value| substitute_immutable_values(value, source_form, environment))
                .collect::<Result<_, _>>()?,
            span: *span,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn expand_expression(
    expression: &crate::ExpressionSyntax,
    source_span: crate::Span,
    left: Option<&Stage>,
    right: &[PendingStage],
    source_form: &CheckedCanonicalForm,
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    catalog: &ProfileCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    gears: &mut Vec<CheckedGear>,
    provenance: &mut Vec<ExpandedGearProvenance>,
    gear_ids: &mut BTreeSet<GearId>,
    anonymous_counts: &mut BTreeMap<String, usize>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    let source = left
        .and_then(|stage| stage.output.as_ref())
        .ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                format!(
                    "pure expression at {}:{} requires one typed input",
                    source_span.line, source_span.column
                ),
            )
        })?;
    let (input_kind, temporal) = match source {
        StageSource::Internal(endpoint) => {
            (endpoint.port.value_kind.clone(), endpoint.port.temporal)
        }
        StageSource::FaceInput(_, kind, temporal) => (kind.clone(), *temporal),
    };
    if let Some(right_temporal) = right.iter().find_map(|stage| match stage {
        PendingStage::Ready(stage) => input_temporal(stage),
        PendingStage::Selector { .. } | PendingStage::Expression { .. } => None,
    }) {
        if right_temporal != temporal {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "pure expression cannot change a Cord's temporal contract".into(),
            ));
        }
    }

    let input_type = crate::CheckedExpressionType::Semantic(input_kind);
    let expression = substitute_immutable_values(expression, source_form, environment)?;
    let immutable_values = BTreeMap::new();
    let literal_types = BTreeMap::new();
    let numeric_types = BTreeSet::new();
    let semantic_kinds = catalog
        .canonical_kinds()
        .values()
        .cloned()
        .map(|kind| (kind.kind_id.as_str().to_string(), kind))
        .collect::<BTreeMap<_, _>>();
    let checked = crate::check_expression(
        &expression,
        &crate::ExpressionTypeContext {
            input: &input_type,
            immutable_values: &immutable_values,
            structured_types,
            literal_types: &literal_types,
            numeric_types: &numeric_types,
            semantic_kinds: &semantic_kinds,
        },
    )
    .map_err(|diagnostic| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            format!(
                "pure expression at {}:{} is not well typed: {}",
                diagnostic.span.line, diagnostic.span.column, diagnostic.message
            ),
        )
    })?;
    let definition = crate::pure_expression_definition(&checked, temporal).map_err(|_| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            "pure expression has no finite exact Port identity".into(),
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
    let configuration = definition
        .configuration
        .iter()
        .map(|field| conduit_core::ConfigurationEntry {
            key: field.key.clone(),
            value: field.default_value.clone(),
        })
        .collect();
    gears.push(CheckedGear {
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
        configuration,
        pool_references: Vec::new(),
    });
    provenance.push(ExpandedGearProvenance {
        gear_id: gear_id.as_str().to_string(),
        form_path: path.to_vec(),
        source_form: source_form.name.clone(),
        source_gear: name,
        source_span,
    });
    Ok(Stage {
        input: Some(vec![StageSink::Internal(Endpoint {
            gear_id: gear_id.clone(),
            port: input,
        })]),
        output: Some(StageSource::Internal(Endpoint {
            gear_id,
            port: output,
        })),
    })
}
