//! Expand one checked Source instance while retaining its callable boundaries.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn expand_instance_inner(
    plot: &CheckedCanonicalPlot,
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
) -> Result<Fragment, CanonicalExpansionDiagnostic> {
    let scoped_environment = bind_pool_environment(plot, environment, path)?;
    let environment = &scoped_environment;
    let mut gears = Vec::new();
    let mut connections = ConnectionInventory::default();
    let mut shared_pools = expanded_pool_declarations(plot, path);
    let mut provenance = Vec::new();
    let mut activations = Vec::new();
    let mut instances = BTreeMap::new();
    let mut gear_ids = BTreeSet::new();
    for gear in plot.gears.iter().filter(|gear| gear.name.is_some()) {
        let name = gear.name.as_deref().expect("named gears were filtered");
        let instance = instantiate_gear(
            gear,
            None,
            name,
            plot,
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
            &mut gears,
            &mut connections,
            &mut shared_pools,
            &mut provenance,
            &mut activations,
            &mut gear_ids,
        )?;
        instances.insert(name.to_string(), instance);
    }

    let runtime_front = plot.checked_front();
    let front_ports = checked_front_ports(plot, &runtime_front);
    let mut inputs = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    let mut anonymous_counts = BTreeMap::<String, usize>::new();
    for cord in &plot.cords {
        let connection_start = connections.len();
        let mut pending = Vec::with_capacity(cord.stages.len());
        for cord_stage in &cord.stages {
            pending.push(match cord_stage {
                CheckedCordStage::Reference(reference) => structured_selector::PendingStage::Ready(
                    resolve_reference(reference, &instances, &front_ports)?,
                ),
                CheckedCordStage::RelationalGear {
                    operands,
                    gear,
                    input_ports,
                    output_port,
                } => {
                    let key = inline_key(gear);
                    let count = anonymous_counts.entry(key.clone()).or_default();
                    let name = format!("inline-{}-{count}", &hash_string(&key)[..12]);
                    *count += 1;
                    let instance = instantiate_gear(
                        gear,
                        Some(operands.len()),
                        &name,
                        plot,
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
                        &mut gears,
                        &mut connections,
                        &mut shared_pools,
                        &mut provenance,
                        &mut activations,
                        &mut gear_ids,
                    )?;
                    for (operand, input_port) in operands.iter().zip(input_ports) {
                        let source = resolve_reference(operand, &instances, &front_ports)?
                            .output
                            .ok_or_else(|| {
                                CanonicalExpansionDiagnostic::new(
                                    "CND-FRM-036",
                                    format!("glyph operand '{operand}' has no output"),
                                )
                            })?;
                        let sinks = instance.inputs.get(input_port).ok_or_else(|| {
                            CanonicalExpansionDiagnostic::new(
                                "CND-FRM-043",
                                format!(
                                    "glyph Gear '{}' has no checked input port '{}'",
                                    gear.kind, input_port
                                ),
                            )
                        })?;
                        for sink in sinks {
                            connect(
                                source.clone(),
                                StageSink::Internal(sink.clone()),
                                crate::ExpandedConnectionOrigin::RelationalOperand,
                                cord.source_span,
                                &mut connections,
                                &mut inputs,
                                &mut outputs,
                            )?;
                        }
                    }
                    let output = instance.outputs.get(output_port).cloned().ok_or_else(|| {
                        CanonicalExpansionDiagnostic::new(
                            "CND-FRM-043",
                            format!(
                                "glyph Gear '{}' has no checked output port '{}'",
                                gear.kind, output_port
                            ),
                        )
                    })?;
                    structured_selector::PendingStage::Ready(Stage {
                        input: None,
                        output: Some(StageSource::Internal(output)),
                    })
                }
                CheckedCordStage::TerminalProjection {
                    endpoint,
                    terminal,
                    source_span,
                } => structured_selector::PendingStage::Ready(project_terminal(
                    resolve_terminal_reference(endpoint, *terminal, &instances, &front_ports)?,
                    *terminal,
                    *source_span,
                )?),
                CheckedCordStage::Cancellation { gear, source_span } => {
                    structured_selector::PendingStage::Ready(cancellation_sink(
                        gear,
                        &instances,
                        *source_span,
                    )?)
                }
                CheckedCordStage::When {
                    expression,
                    source_span,
                } => structured_selector::PendingStage::When {
                    expression: expression.clone(),
                    source_span: *source_span,
                },
                CheckedCordStage::PureExpression {
                    expression,
                    source_span,
                } => structured_selector::PendingStage::Expression {
                    expression: expression.clone(),
                    source_span: *source_span,
                },
                CheckedCordStage::InlineGear(gear) => {
                    let key = inline_key(gear);
                    let count = anonymous_counts.entry(key.clone()).or_default();
                    let name = format!("inline-{}-{count}", &hash_string(&key)[..12]);
                    *count += 1;
                    let instance = instantiate_gear(
                        gear,
                        None,
                        &name,
                        plot,
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
                        &mut gears,
                        &mut connections,
                        &mut shared_pools,
                        &mut provenance,
                        &mut activations,
                        &mut gear_ids,
                    )?;
                    structured_selector::PendingStage::Ready(stage_for_instance(
                        &name, &instance, None,
                    )?)
                }
                CheckedCordStage::Literal { value, source_span } => {
                    structured_selector::PendingStage::Ready(literal::expand_literal(
                        value,
                        *source_span,
                        plot,
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
                        &mut gears,
                        &mut connections,
                        &mut shared_pools,
                        &mut provenance,
                        &mut gear_ids,
                        &mut anonymous_counts,
                    )?)
                }
                CheckedCordStage::StructuredSelector {
                    selector,
                    source_span,
                } => structured_selector::PendingStage::Selector {
                    selector: selector.clone(),
                    source_span: *source_span,
                },
            });
        }
        let stages = structured_selector::resolve_selectors(
            pending,
            plot,
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
            &mut gears,
            &mut connections,
            &mut shared_pools,
            &mut provenance,
            &mut gear_ids,
            &mut anonymous_counts,
        )?;
        for pair in stages.windows(2) {
            let source = pair[0].output.clone().ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-036",
                    "cord stage has no output; use an explicit output port".into(),
                )
            })?;
            let sinks = pair[1].input.clone().ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-036",
                    "cord stage has no input; use an explicit input port".into(),
                )
            })?;
            for sink in sinks {
                connect(
                    source.clone(),
                    sink,
                    crate::ExpandedConnectionOrigin::CordStages,
                    cord.source_span,
                    &mut connections,
                    &mut inputs,
                    &mut outputs,
                )?;
            }
        }
        connections.attribute_since(connection_start, plot, path, cord.source_span);
    }
    validate_front_bindings(plot, &inputs, &outputs)?;
    qualified_front::validate(plot, &inputs, &outputs, &gears)?;
    qualified_front::validate_connections(&gears, &connections)?;
    if let Some((input, output)) = &plot.shorthand {
        if !inputs.contains_key(input) || !outputs.contains_key(output) {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-044",
                format!(
                    "plot '{}' shorthand path does not bind its declared input and output",
                    plot.name
                ),
            ));
        }
    }
    let abnormal = infer_abnormal_export(&gears, &connections, &outputs)?;
    Ok(Fragment {
        gears,
        connections,
        shared_pools,
        provenance,
        activations,
        inputs,
        outputs,
        abnormal,
        shorthand: plot.shorthand.clone(),
    })
}
