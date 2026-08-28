use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn expand_literal(
    value: &CanonicalStartupValue,
    source_span: crate::Span,
    source_form: &CheckedCanonicalForm,
    forms: &BTreeMap<&str, &CheckedCanonicalForm>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::RealizationBack>,
    depth: usize,
    gears: &mut Vec<CheckedGear>,
    connections: &mut Vec<CheckedConnection>,
    shared_pools: &mut Vec<ExpandedSharedPool>,
    provenance: &mut Vec<ExpandedGearProvenance>,
    gear_ids: &mut BTreeSet<GearId>,
    anonymous_counts: &mut BTreeMap<String, usize>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    let CanonicalStartupValue::Literal(literal) = value else {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            "runtime literal stage did not retain an immutable literal".into(),
        ));
    };
    let key = format!("text/literal:{}:{literal}", literal.len());
    let count = anonymous_counts.entry(key.clone()).or_default();
    let name = format!("literal-{}-{count}", &hash_string(&key)[..12]);
    *count += 1;
    let gear = CheckedCanonicalGear {
        name: None,
        kind: "text/literal".to_string(),
        startup_parameters: vec![crate::StartupParameterSignature {
            name: "value".to_string(),
            value_type: "Text".to_string(),
            default: None,
        }],
        startup_bindings: vec![crate::CheckedStartupBinding {
            name: "value".to_string(),
            value_type: "Text".to_string(),
            value: value.clone(),
        }],
        source_span,
    };
    let instance = instantiate_gear(
        &gear,
        &name,
        source_form,
        forms,
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
    stage_for_instance(&name, &instance, None)
}
