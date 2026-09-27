use super::*;

pub(super) fn semantic_call_chain(
    expression: &crate::ExpressionSyntax,
) -> Option<Vec<crate::SpannedText>> {
    match expression {
        crate::ExpressionSyntax::Input(_) => Some(Vec::new()),
        crate::ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } if arguments.len() == 1 => {
            let mut calls = semantic_call_chain(&arguments[0])?;
            calls.push(kind.clone());
            Some(calls)
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn expand_semantic_call_chain(
    calls: &[crate::SpannedText],
    source_span: crate::Span,
    source_form: &CheckedCanonicalForm,
    catalog: &ProfileCatalog,
    path: &[String],
    gears: &mut Vec<CheckedGear>,
    connections: &mut Vec<CheckedConnection>,
    provenance: &mut Vec<ExpandedGearProvenance>,
    gear_ids: &mut BTreeSet<GearId>,
    anonymous_counts: &mut BTreeMap<String, usize>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    let mut stages = calls
        .iter()
        .map(|kind| {
            expand_one(
                kind,
                source_span,
                source_form,
                catalog,
                path,
                gears,
                provenance,
                gear_ids,
                anonymous_counts,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    for pair in stages.windows(2) {
        let (StageSource::Internal(source), StageSink::Internal(sink)) = (
            pair[0]
                .output
                .clone()
                .expect("semantic call stage has one output"),
            pair[1]
                .input
                .as_ref()
                .and_then(|inputs| inputs.first())
                .cloned()
                .expect("semantic call stage has one input"),
        ) else {
            unreachable!("semantic call chain uses internal ports")
        };
        connections.push(CheckedConnection {
            source_gear_id: source.gear_id,
            source_port_id: source.port.port_id,
            sink_gear_id: sink.gear_id,
            sink_port_id: sink.port.port_id,
            value_kind: source.port.value_kind,
            temporal: source.port.temporal,
        });
    }
    let first = stages.remove(0);
    let last = stages.pop().unwrap_or_else(|| first.clone());
    Ok(Stage {
        input: first.input,
        output: last.output,
    })
}

#[allow(clippy::too_many_arguments)]
fn expand_one(
    kind_name: &crate::SpannedText,
    source_span: crate::Span,
    source_form: &CheckedCanonicalForm,
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
    let input = kind.inputs[0].clone();
    let output = kind.outputs[0].clone();
    gears.push(CheckedGear {
        gear_id: gear_id.clone(),
        kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        startup_parameters: kind.startup_parameters.clone(),
        shorthand: kind.shorthand.clone(),
        inputs: kind.inputs.clone(),
        outputs: kind.outputs.clone(),
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
