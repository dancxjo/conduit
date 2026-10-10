use super::*;
use crate::{KindConfigurationField, KindConfigurationRule, KindProjection};
use conduit_core::{
    ConfigurationValue, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
    StructuredSelector, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

mod call_structure;
mod physical_literals;
mod semantic_call;
mod substitution;
mod temporal;
mod when_filter;
use call_structure::contains_semantic_call;
use semantic_call::expand_semantic_call_graph;
use substitution::substitute_immutable_values;
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
            abnormal_kind: None,
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
            abnormal_kind: None,
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
    When {
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
    source_plot: &CheckedCanonicalPlot,
    plots: &BTreeMap<&str, &CheckedCanonicalPlot>,
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    exact_initial_info: &BTreeMap<(conduit_core::KindId, String), Vec<u8>>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::PlotBack>,
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
                    source_plot,
                    structured_types,
                    catalog,
                    environment,
                    path,
                    gears,
                    connections,
                    provenance,
                    gear_ids,
                    anonymous_counts,
                )?),
                PendingStage::When {
                    expression,
                    source_span,
                } => stages.push(when_filter::expand_when_filter(
                    expression,
                    *source_span,
                    stages.last(),
                    &pending[index + 1..],
                    source_plot,
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
            PendingStage::Selector { .. }
            | PendingStage::Expression { .. }
            | PendingStage::When { .. } => None,
        });
        let right = pending[index + 1..].iter().find_map(|stage| match stage {
            PendingStage::Ready(stage) => input_temporal(stage),
            PendingStage::Selector { .. }
            | PendingStage::Expression { .. }
            | PendingStage::When { .. } => None,
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
            activation: None,
            source_span: *source_span,
        };
        let mut generated_activations = Vec::new();
        let instance = instantiate_gear(
            &gear,
            None,
            &name,
            source_plot,
            plots,
            structured_types,
            exact_initial_info,
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
            &mut generated_activations,
            gear_ids,
        )?;
        stages.push(stage_for_instance(&name, &instance, None)?);
    }
    Ok(stages)
}

#[allow(clippy::too_many_arguments)]
fn expand_expression(
    expression: &crate::ExpressionSyntax,
    source_span: crate::Span,
    left: Option<&Stage>,
    right: &[PendingStage],
    source_plot: &CheckedCanonicalPlot,
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    catalog: &ProfileCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    gears: &mut Vec<CheckedGear>,
    connections: &mut Vec<CheckedConnection>,
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
        StageSource::FaceInput(_, kind, temporal, _, _) => (kind.clone(), *temporal),
    };
    let right_stage = right.iter().find_map(|stage| match stage {
        PendingStage::Ready(stage) => Some(stage),
        PendingStage::Selector { .. }
        | PendingStage::Expression { .. }
        | PendingStage::When { .. } => None,
    });
    if let Some(right_temporal) = right_stage.and_then(input_temporal) {
        if right_temporal != temporal {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "pure expression cannot change a Cord's temporal contract".into(),
            ));
        }
    }

    let input_type = crate::CheckedExpressionType::Semantic(input_kind);
    let mut expression = substitute_immutable_values(expression, source_plot, environment)?;
    let (literal_types, canonical_literals) =
        physical_literals::bind(&mut expression, source_plot, environment, catalog)?;
    let immutable_values = BTreeMap::new();
    let numeric_types = BTreeSet::new();
    let semantic_kinds = catalog
        .canonical_kinds()
        .values()
        .cloned()
        .map(|kind| (kind.kind_id.as_str().to_string(), kind))
        .collect::<BTreeMap<_, _>>();
    let expected_output = right_stage
        .and_then(|stage| stage.input.as_ref())
        .and_then(|inputs| inputs.first())
        .map(|sink| match sink {
            StageSink::Internal(endpoint) => endpoint.port.value_kind.clone(),
            StageSink::FaceOutput(_, kind, _, _) => kind.clone(),
        })
        .map(crate::CheckedExpressionType::Semantic);
    let mut checked = crate::expression_check::check_expression_as(
        &expression,
        expected_output.as_ref(),
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
    checked.canonical_literals = canonical_literals;
    if let Some(invariants) = catalog.type_invariants(
        checked
            .input_type
            .value_kind()
            .expect("Cord input has one exact semantic Kind"),
    ) {
        crate::expression_proof::apply(&mut checked, invariants);
    }
    if temporal == PortTemporal::Value && contains_semantic_call(&expression) {
        return expand_semantic_call_graph(
            &expression,
            input_type
                .value_kind()
                .expect("Cord input has one exact semantic Kind"),
            expected_output
                .as_ref()
                .and_then(crate::CheckedExpressionType::value_kind),
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
    let definition = crate::pure_expression_definition(&checked, temporal).map_err(|_| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            "pure expression has no finite exact Port identity".into(),
        )
    })?;
    drop(checked);
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
        configuration,
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
