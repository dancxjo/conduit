use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn expand_direct_semantic_call(
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
