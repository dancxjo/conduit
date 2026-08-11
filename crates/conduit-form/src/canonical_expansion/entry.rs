use super::*;

pub fn expand_canonical_form(
    document: &CheckedSyntaxDocument,
    form_name: &str,
    catalog: &ProfileCatalog,
) -> Result<ExpandedCanonicalForm, CanonicalExpansionDiagnostic> {
    expand_canonical_form_with_backs(document, form_name, catalog, &CanonicalBackCatalog::new())
}

pub fn expand_canonical_form_with_backs(
    document: &CheckedSyntaxDocument,
    form_name: &str,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
) -> Result<ExpandedCanonicalForm, CanonicalExpansionDiagnostic> {
    let forms = document
        .forms
        .iter()
        .map(|form| (form.name.as_str(), form))
        .collect::<BTreeMap<_, _>>();
    let form = forms.get(form_name).copied().ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-031",
            format!("canonical form '{form_name}' is not defined"),
        )
    })?;
    let mut environment = BTreeMap::new();
    for parameter in &form.startup_parameters {
        let value = parameter.default.clone().ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-032",
                format!(
                    "root form '{form_name}' requires startup parameter '{}'",
                    parameter.name
                ),
            )
        })?;
        environment.insert(parameter.name.clone(), value);
    }
    let mut stack = Vec::new();
    let mut realization_backs = Vec::new();
    let fragment = expand_instance(
        form,
        &forms,
        catalog,
        backs,
        &environment,
        core::slice::from_ref(&form.name),
        &mut stack,
        &mut realization_backs,
        0,
    )?;
    if !fragment.inputs.is_empty() || !fragment.outputs.is_empty() {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-033",
            format!("root form '{form_name}' has unbound runtime face ports"),
        ));
    }
    let mut gears = fragment.gears;
    let mut connections = fragment.connections;
    let mut shared_pools = fragment.shared_pools;
    let mut provenance = fragment.provenance;
    gears.sort_by(|left, right| left.gear_id.cmp(&right.gear_id));
    connections.sort_by(|left, right| {
        (
            &left.source_gear_id,
            &left.source_port_id,
            &left.sink_gear_id,
            &left.sink_port_id,
        )
            .cmp(&(
                &right.source_gear_id,
                &right.source_port_id,
                &right.sink_gear_id,
                &right.sink_port_id,
            ))
    });
    provenance.sort_by(|left, right| left.gear_id.cmp(&right.gear_id));
    seal_pool_consumers(&mut shared_pools, &gears)?;
    realization_backs.sort();
    let expanded_form_id = expanded_identity(
        form,
        &gears,
        &connections,
        &shared_pools,
        &provenance,
        &realization_backs,
    );
    let provenance_digest = provenance_digest(&document.source_document_id, &provenance);
    Ok(ExpandedCanonicalForm {
        source_document_id: document.source_document_id.clone(),
        checked_form_id: form.checked_form_id.clone(),
        expanded_form_id,
        name: form.name.clone(),
        gears,
        connections,
        shared_pools,
        provenance,
        provenance_digest,
        realization_backs,
    })
}
