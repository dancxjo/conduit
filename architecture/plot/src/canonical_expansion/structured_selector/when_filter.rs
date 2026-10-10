use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn expand_when_filter(
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
    provenance: &mut Vec<ExpandedGearProvenance>,
    gear_ids: &mut BTreeSet<GearId>,
    anonymous_counts: &mut BTreeMap<String, usize>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    let source = left
        .and_then(|stage| stage.output.as_ref())
        .ok_or_else(|| diagnostic(source_span, "when filter requires one typed input"))?;
    let (input_kind, temporal) = match source {
        StageSource::Internal(endpoint) => {
            (endpoint.port.value_kind.clone(), endpoint.port.temporal)
        }
        StageSource::FaceInput(_, kind, temporal, _, _) => (kind.clone(), *temporal),
    };
    if let Some(right_temporal) = right.iter().find_map(|stage| match stage {
        PendingStage::Ready(stage) => input_temporal(stage),
        PendingStage::Selector { .. }
        | PendingStage::Expression { .. }
        | PendingStage::When { .. } => None,
    }) {
        if right_temporal != temporal {
            return Err(diagnostic(
                source_span,
                "when filter cannot change a Cord's temporal contract",
            ));
        }
    }
    let mut expression = substitute_immutable_values(expression, source_plot, environment)?;
    let (literal_types, canonical_literals) =
        physical_literals::bind(&mut expression, source_plot, environment, catalog)?;
    let semantic_kinds = catalog
        .canonical_kinds()
        .values()
        .cloned()
        .map(|kind| (kind.kind_id.as_str().to_string(), kind))
        .collect::<BTreeMap<_, _>>();
    let mut checked = crate::check_expression(
        &expression,
        &crate::ExpressionTypeContext {
            input: &crate::CheckedExpressionType::Semantic(input_kind),
            immutable_values: &BTreeMap::new(),
            structured_types,
            literal_types: &literal_types,
            numeric_types: &BTreeSet::new(),
            semantic_kinds: &semantic_kinds,
        },
    )
    .map_err(|error| {
        diagnostic(
            error.span,
            &format!("when predicate is not well typed: {}", error.message),
        )
    })?;
    checked.canonical_literals = canonical_literals;
    let definition = crate::pure_filter_definition(&checked, temporal).map_err(|_| {
        diagnostic(
            source_span,
            "when predicate must return exactly Boolean and have a finite portable program",
        )
    })?;
    let key = definition.kind_id.as_str().to_string();
    let count = anonymous_counts.entry(key.clone()).or_default();
    let name = format!("filter-{}-{count}", &hash_string(&key)[..12]);
    *count += 1;
    let mut child_path = path.to_vec();
    child_path.push(name.clone());
    let gear_id = GearId::from(child_path.join("/"));
    if !gear_ids.insert(gear_id.clone()) {
        return Err(diagnostic(
            source_span,
            "expanded when filter path is not unique",
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

fn diagnostic(span: crate::Span, message: &str) -> CanonicalExpansionDiagnostic {
    CanonicalExpansionDiagnostic::new(
        "CND-FRM-046",
        format!("{} at {}:{}", message, span.line, span.column),
    )
}
