use super::*;

pub fn expand_canonical_plot(
    document: &CheckedSyntaxDocument,
    plot_name: &str,
    catalog: &ProfileCatalog,
) -> Result<ExpandedCanonicalPlot, CanonicalExpansionDiagnostic> {
    expand_canonical_plot_with_backs(document, plot_name, catalog, &CanonicalBackCatalog::new())
}

pub fn expand_canonical_plot_with_backs(
    document: &CheckedSyntaxDocument,
    plot_name: &str,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
) -> Result<ExpandedCanonicalPlot, CanonicalExpansionDiagnostic> {
    let authoring =
        expand_canonical_plot_for_authoring_with_backs(document, plot_name, catalog, backs)?;
    if !authoring.input_bindings.is_empty() || !authoring.output_bindings.is_empty() {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-033",
            format!("root plot '{plot_name}' has unbound runtime front ports"),
        ));
    }
    Ok(authoring.expanded)
}

pub fn expand_canonical_plot_for_authoring(
    document: &CheckedSyntaxDocument,
    plot_name: &str,
    catalog: &ProfileCatalog,
) -> Result<ExpandedAuthoringPlot, CanonicalExpansionDiagnostic> {
    expand_canonical_plot_for_authoring_with_backs(
        document,
        plot_name,
        catalog,
        &CanonicalBackCatalog::new(),
    )
}

pub fn expand_canonical_plot_for_authoring_with_backs(
    document: &CheckedSyntaxDocument,
    plot_name: &str,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
) -> Result<ExpandedAuthoringPlot, CanonicalExpansionDiagnostic> {
    let plots = document
        .plots
        .iter()
        .map(|plot| (plot.name.as_str(), plot))
        .collect::<BTreeMap<_, _>>();
    let plot = plots.get(plot_name).copied().ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-031",
            format!("canonical plot '{plot_name}' is not defined"),
        )
    })?;
    if crate::syntax_check::is_local_plot_identity(plot_name) {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-061",
            "a local Plot is private to its containing source scope and cannot be expanded as a root"
                .into(),
        ));
    }
    let mut environment = BTreeMap::new();
    for parameter in &plot.startup_parameters {
        let value = parameter.default.clone().ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-032",
                format!(
                    "root plot '{plot_name}' requires startup parameter '{}'",
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
        plot,
        &plots,
        document.structured_types(),
        &catalog,
        backs,
        &environment,
        core::slice::from_ref(&plot.name),
        &mut stack,
        &mut realization_backs,
        0,
    )?;
    let front = plot.checked_front();
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
            .map(|endpoint| crate::CheckedPlotAbnormalExport {
                value_kind: endpoint
                    .port
                    .abnormal_kind
                    .clone()
                    .expect("inferred abnormal export has an exact terminal Kind"),
                gear_id: endpoint.gear_id.clone(),
                gear_port_id: endpoint.port.port_id.clone(),
            });
    let mut gears = fragment.gears;
    super::construction::validate(&gears, &document.retained_native_types)?;
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
    let expanded_plot_id = expanded_identity(
        plot,
        &gears,
        &connections,
        &shared_pools,
        &provenance,
        &realization_backs,
        &activations,
    );
    let provenance_digest = provenance_digest(&document.source_document_id, &provenance);
    Ok(ExpandedAuthoringPlot {
        expanded: ExpandedCanonicalPlot {
            source_document_id: document.source_document_id.clone(),
            checked_plot_id: plot.checked_plot_id.clone(),
            expanded_plot_id,
            name: plot.name.clone(),
            completion: plot.completion,
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
