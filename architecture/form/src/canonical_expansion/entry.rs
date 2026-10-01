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
    let authoring =
        expand_canonical_form_for_authoring_with_backs(document, form_name, catalog, backs)?;
    if !authoring.input_bindings.is_empty() || !authoring.output_bindings.is_empty() {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-033",
            format!("root form '{form_name}' has unbound runtime front ports"),
        ));
    }
    Ok(authoring.expanded)
}

pub fn expand_canonical_form_for_authoring(
    document: &CheckedSyntaxDocument,
    form_name: &str,
    catalog: &ProfileCatalog,
) -> Result<ExpandedAuthoringForm, CanonicalExpansionDiagnostic> {
    expand_canonical_form_for_authoring_with_backs(
        document,
        form_name,
        catalog,
        &CanonicalBackCatalog::new(),
    )
}

pub fn expand_canonical_form_for_authoring_with_backs(
    document: &CheckedSyntaxDocument,
    form_name: &str,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
) -> Result<ExpandedAuthoringForm, CanonicalExpansionDiagnostic> {
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
    if crate::syntax_check::is_local_form_identity(form_name) {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-061",
            "a local Form is private to its containing source scope and cannot be expanded as a root"
                .into(),
        ));
    }
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
    let mut catalog = catalog.clone();
    catalog.install_type_invariants(&document.native_types);
    let fragment = expand_instance(
        form,
        &forms,
        document.structured_types(),
        &catalog,
        backs,
        &environment,
        core::slice::from_ref(&form.name),
        &mut stack,
        &mut realization_backs,
        0,
    )?;
    let front = form.checked_front();
    let input_bindings = fragment
        .inputs
        .iter()
        .flat_map(|(front_port, endpoints)| {
            endpoints.iter().map(|endpoint| AuthoringFrontBinding {
                front_port_id: conduit_core::PortId::from(front_port.as_str()),
                gear_id: endpoint.endpoint.gear_id.clone(),
                gear_port_id: endpoint.endpoint.port.port_id.clone(),
                track: endpoint.track,
            })
        })
        .collect();
    let output_bindings = fragment
        .outputs
        .iter()
        .map(|(front_port, endpoint)| AuthoringFrontBinding {
            front_port_id: conduit_core::PortId::from(front_port.as_str()),
            gear_id: endpoint.endpoint.gear_id.clone(),
            gear_port_id: endpoint.endpoint.port.port_id.clone(),
            track: endpoint.track,
        })
        .collect();
    let abnormal_export =
        fragment
            .abnormal
            .as_ref()
            .map(|endpoint| crate::CheckedFormAbnormalExport {
                value_kind: endpoint
                    .port
                    .abnormal_kind
                    .clone()
                    .expect("inferred abnormal export has an exact terminal Kind"),
                gear_id: endpoint.gear_id.clone(),
                gear_port_id: endpoint.port.port_id.clone(),
            });
    let mut gears = fragment.gears;
    let mut connections = fragment.connections;
    let mut shared_pools = fragment.shared_pools;
    let mut provenance = fragment.provenance;
    let mut activations = fragment.activations;
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
    activations.sort_by(|left, right| left.activation_id.cmp(&right.activation_id));
    seal_pool_consumers(&mut shared_pools, &gears)?;
    realization_backs.sort();
    let expanded_form_id = expanded_identity(
        form,
        &gears,
        &connections,
        &shared_pools,
        &provenance,
        &realization_backs,
        &activations,
    );
    let provenance_digest = provenance_digest(&document.source_document_id, &provenance);
    Ok(ExpandedAuthoringForm {
        expanded: ExpandedCanonicalForm {
            source_document_id: document.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
            expanded_form_id,
            name: form.name.clone(),
            completion: form.completion,
            gears,
            connections,
            shared_pools,
            provenance,
            provenance_digest,
            realization_backs,
            activations,
        },
        front,
        input_bindings,
        output_bindings,
        abnormal_export,
    })
}
