use super::*;

pub(super) fn checked_front_ports<'a>(
    plot: &'a CheckedCanonicalPlot,
    runtime_front: &'a conduit_core::CheckedFront,
) -> BTreeMap<&'a str, (&'a crate::RuntimePort, &'a conduit_core::PortDescriptor)> {
    let descriptors = runtime_front
        .inputs()
        .iter()
        .chain(runtime_front.outputs())
        .map(|port| (port.port_id.as_str(), port))
        .collect::<BTreeMap<_, _>>();
    plot.runtime_ports
        .iter()
        .map(|port| {
            let descriptor = descriptors
                .get(port.name.text.as_str())
                .expect("checked runtime front retains every syntax Port");
            (port.name.text.as_str(), (port, *descriptor))
        })
        .collect()
}

pub(super) fn inline_key(gear: &CheckedCanonicalGear) -> String {
    format!("{}:{:?}", gear.kind, gear.startup_bindings)
}

pub(super) fn configuration(
    gear: &CheckedCanonicalGear,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    definition: &crate::KindProjection,
) -> Result<Vec<conduit_core::ConfigurationEntry>, CanonicalExpansionDiagnostic> {
    for binding in &gear.startup_bindings {
        if definition
            .configuration
            .iter()
            .any(|field| field.key == binding.name)
        {
            continue;
        }
        if !matches!(
            substitute(&binding.value, environment)?,
            CanonicalStartupValue::PoolReference(_)
        ) {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                format!(
                    "startup parameter '{}' has no exact primitive planning field",
                    binding.name
                ),
            ));
        }
    }
    definition
        .configuration
        .iter()
        .map(|field| {
            let value = gear
                .startup_bindings
                .iter()
                .find(|binding| binding.name == field.key)
                .ok_or_else(|| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-041",
                        format!(
                            "primitive planning field '{}' is absent from the checked startup signature",
                            field.key
                        ),
                    )
                })
                .and_then(|binding| substitute(&binding.value, environment))
                .and_then(|value| validate_startup_configuration(field, value))?;
            Ok(conduit_core::ConfigurationEntry {
                key: field.key.clone(),
                value,
            })
        })
        .collect()
}

pub(super) fn pool_references(
    gear: &CheckedCanonicalGear,
    environment: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<Vec<conduit_core::SharedPoolId>, CanonicalExpansionDiagnostic> {
    let mut pools = gear
        .startup_bindings
        .iter()
        .map(|binding| substitute(&binding.value, environment))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|value| match value {
            CanonicalStartupValue::PoolReference(pool) => Some(pool),
            CanonicalStartupValue::Literal(_)
            | CanonicalStartupValue::Quantity(_)
            | CanonicalStartupValue::Unit(_)
            | CanonicalStartupValue::TemperatureDifference(_)
            | CanonicalStartupValue::PlotParameter(_)
            | CanonicalStartupValue::Structured(_) => None,
        })
        .collect::<Vec<_>>();
    pools.sort();
    pools.dedup();
    Ok(pools)
}

pub(super) fn resolve_reference(
    reference: &str,
    instances: &BTreeMap<String, Instance>,
    front_ports: &BTreeMap<&str, (&crate::RuntimePort, &conduit_core::PortDescriptor)>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    if let Some((port, descriptor)) = front_ports.get(reference) {
        return Ok(match port.direction {
            RuntimePortDirection::Input => Stage {
                input: None,
                output: Some(StageSource::FaceInput(
                    reference.to_string(),
                    descriptor.value_kind.clone(),
                    crate::value_type::canonical_port_temporal(port.temporal),
                    descriptor.abnormal_kind.clone(),
                    conduit_core::ConnectionTrack::Payload,
                )),
            },
            RuntimePortDirection::Output => Stage {
                input: Some(vec![StageSink::FaceOutput(
                    reference.to_string(),
                    descriptor.value_kind.clone(),
                    crate::value_type::canonical_port_temporal(port.temporal),
                    descriptor.abnormal_kind.clone(),
                )]),
                output: None,
            },
        });
    }
    let (instance_name, explicit_port) = reference
        .split_once('.')
        .map_or((reference, None), |(name, port)| (name, Some(port)));
    let instance = instances.get(instance_name).ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-042",
            format!("cord references unknown gear or front port '{reference}'"),
        )
    })?;
    stage_for_instance(instance_name, instance, explicit_port)
}

pub(super) fn resolve_terminal_reference(
    reference: &str,
    terminal: crate::TerminalProjection,
    instances: &BTreeMap<String, Instance>,
    front_ports: &BTreeMap<&str, (&crate::RuntimePort, &conduit_core::PortDescriptor)>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    if terminal == crate::TerminalProjection::Abnormal && !reference.contains('.') {
        if let Some(endpoint) = instances
            .get(reference)
            .and_then(|instance| instance.abnormal.clone())
        {
            return Ok(Stage {
                input: None,
                output: Some(StageSource::Internal(endpoint)),
            });
        }
    }
    resolve_reference(reference, instances, front_ports)
}

/// Infer the one exact abnormal terminal which the containing Plot must expose.
///
/// An exact `Recover` transduction discharges its source only after successful
/// normal completion at runtime. Observation and ordinary abnormal routing do
/// not. Multiple remaining origins cannot be collapsed without semantic fan-in,
/// so expansion refuses rather than inventing an error bus.
pub(super) fn infer_abnormal_export(
    gears: &[CheckedGear],
    connections: &[CheckedConnection],
    outputs: &BTreeMap<String, TrackedEndpoint>,
) -> Result<Option<TrackedEndpoint>, CanonicalExpansionDiagnostic> {
    let recovery_edges = connections
        .iter()
        .filter(|connection| {
            if connection.track != conduit_core::ConnectionTrack::AbnormalTerminal {
                return false;
            }
            gears
                .iter()
                .find(|gear| gear.gear_id == connection.sink_gear_id)
                .and_then(|gear| {
                    gear.terminal_transductions
                        .iter()
                        .find(|contract| contract.input_port_id == connection.sink_port_id)
                })
                .is_some_and(|contract| {
                    contract.input_port_id == connection.sink_port_id
                        && matches!(
                            contract.abnormal,
                            conduit_core::AbnormalTerminalTransduction::Recover
                        )
                })
        })
        .collect::<Vec<_>>();
    for (index, edge) in recovery_edges.iter().enumerate() {
        if recovery_edges[..index].iter().any(|other| {
            (other.sink_gear_id == edge.sink_gear_id && other.sink_port_id == edge.sink_port_id)
                || (other.source_gear_id == edge.source_gear_id
                    && other.source_port_id == edge.source_port_id)
        }) {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-054",
                "projected abnormal recovery must bind one exact source to one exact recovery input"
                    .into(),
            ));
        }
    }
    let mut recovered = recovery_edges
        .iter()
        .map(|connection| (&connection.source_gear_id, &connection.source_port_id))
        .collect::<BTreeSet<_>>();
    recovered.extend(
        outputs
            .values()
            .filter(|endpoint| endpoint.track == conduit_core::ConnectionTrack::AbnormalTerminal)
            .map(|endpoint| (&endpoint.gear_id, &endpoint.port.port_id)),
    );

    let mut unresolved = gears
        .iter()
        .flat_map(|gear| {
            gear.outputs
                .iter()
                .filter(|port| port.abnormal_kind.is_some())
                .filter(|port| !recovered.contains(&(&gear.gear_id, &port.port_id)))
                .map(|port| TrackedEndpoint {
                    endpoint: Endpoint {
                        gear_id: gear.gear_id.clone(),
                        port: port.clone(),
                    },
                    track: conduit_core::ConnectionTrack::AbnormalTerminal,
                })
        })
        .collect::<Vec<_>>();
    unresolved.sort_by(|left, right| {
        (&left.gear_id, &left.port.port_id).cmp(&(&right.gear_id, &right.port.port_id))
    });
    match unresolved.len() {
        0 => Ok(None),
        1 => Ok(unresolved.pop()),
        count => Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-054",
            format!(
                "Plot has {count} unresolved abnormal terminal origins; route exact recovery until one typed containing-Plot terminal remains"
            ),
        )),
    }
}

pub(super) fn stage_for_instance(
    instance_name: &str,
    instance: &Instance,
    explicit_port: Option<&str>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    if let Some(port) = explicit_port {
        let input = instance
            .inputs
            .get(port)
            .map(|endpoints| endpoints.iter().cloned().map(StageSink::Internal).collect());
        let output = instance
            .outputs
            .get(port)
            .cloned()
            .map(StageSource::Internal);
        if input.is_none() && output.is_none() {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-043",
                format!("gear '{instance_name}' has no runtime port '{port}'"),
            ));
        }
        return Ok(Stage { input, output });
    }
    let (input, output) = instance.bare_ports.as_ref().ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-044",
            format!(
                "gear '{instance_name}' has no shorthand front path; name an exact runtime port"
            ),
        )
    })?;
    Ok(Stage {
        input: input
            .as_ref()
            .and_then(|input| instance.inputs.get(input))
            .map(|endpoints| endpoints.iter().cloned().map(StageSink::Internal).collect()),
        output: output
            .as_ref()
            .and_then(|output| instance.outputs.get(output))
            .cloned()
            .map(StageSource::Internal),
    })
}

pub(super) fn project_terminal(
    mut stage: Stage,
    terminal: crate::TerminalProjection,
    source_span: crate::Span,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    let output = stage.output.as_mut().ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            format!(
                "terminal projection at {}:{} requires an output endpoint",
                source_span.line, source_span.column
            ),
        )
    })?;
    let temporal = match output {
        StageSource::Internal(endpoint) => endpoint.port.temporal,
        StageSource::FaceInput(_, _, temporal, _, _) => *temporal,
    };
    let track = match terminal {
        crate::TerminalProjection::NormalClose => {
            if !matches!(temporal, conduit_core::PortTemporal::Flow { closes: true }) {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    format!(
                        "normal-close projection at {}:{} is legal only for a closing Flow",
                        source_span.line, source_span.column
                    ),
                ));
            }
            conduit_core::ConnectionTrack::NormalClose
        }
        crate::TerminalProjection::Abnormal => conduit_core::ConnectionTrack::AbnormalTerminal,
        crate::TerminalProjection::Quiescence => {
            if !matches!(temporal, conduit_core::PortTemporal::Flow { .. }) {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    format!(
                        "quiescence projection at {}:{} is legal only for a Flow",
                        source_span.line, source_span.column
                    ),
                ));
            }
            conduit_core::ConnectionTrack::Quiescence
        }
    };
    match output {
        StageSource::Internal(endpoint) => {
            if terminal == crate::TerminalProjection::Abnormal
                && endpoint.port.abnormal_kind.is_none()
            {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    format!(
                        "abnormal terminal projection at {}:{} requires the source Kind Fore to declare an exact abnormal terminal type for output '{}'",
                        source_span.line,
                        source_span.column,
                        endpoint.port.port_id.as_str()
                    ),
                ));
            }
            endpoint.track = track;
        }
        StageSource::FaceInput(_, _, _, abnormal_kind, projected_track) => {
            if terminal == crate::TerminalProjection::Abnormal && abnormal_kind.is_none() {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-046",
                    format!(
                        "abnormal terminal projection at {}:{} requires an exact abnormal terminal type on the Plot Fore",
                        source_span.line, source_span.column
                    ),
                ));
            }
            *projected_track = track;
        }
    }
    stage.input = None;
    Ok(stage)
}

pub(super) fn cancellation_sink(
    gear: &str,
    instances: &BTreeMap<String, Instance>,
    source_span: crate::Span,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    if gear.contains('.') {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            format!(
                "semantic cancellation at {}:{} names a Gear, not an arbitrary Port",
                source_span.line, source_span.column
            ),
        ));
    }
    let instance = instances.get(gear).ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            format!(
                "semantic cancellation at {}:{} references unknown Gear '{gear}'",
                source_span.line, source_span.column
            ),
        )
    })?;
    if !matches!(
        instance
            .terminal_transductions
            .iter()
            .find_map(|profile| matches!(
                profile.cancellation,
                conduit_core::CancellationTransduction::Request { .. }
            )
            .then_some(&profile.cancellation)),
        Some(conduit_core::CancellationTransduction::Request { .. })
    ) {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            format!(
                "Gear '{gear}' does not declare CancellationTransduction::Request in its exact Kind contract"
            ),
        ));
    }
    let mut cancellation_inputs = instance.inputs.iter().filter(|(_, endpoints)| {
        endpoints.iter().all(|endpoint| {
            endpoint.port.value_kind.as_str() == conduit_core::CANCELLATION_REQUEST_INFO_ID
        })
    });
    let Some((input_name, _)) = cancellation_inputs.next() else {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            format!(
                "Gear '{gear}' does not declare the canonical '{}' Fore control required by semantic cancellation",
                conduit_core::CANCELLATION_REQUEST_INFO_ID
            ),
        ));
    };
    if cancellation_inputs.next().is_some() {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            format!("Gear '{gear}' declares more than one semantic cancellation control"),
        ));
    }
    let mut stage = stage_for_instance(gear, instance, Some(input_name)).map_err(|_| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            format!("Gear '{gear}' has an invalid semantic cancellation Fore control"),
        )
    })?;
    stage.output = None;
    Ok(stage)
}

pub(super) fn connect(
    source: StageSource,
    sink: StageSink,
    connections: &mut Vec<CheckedConnection>,
    inputs: &mut BTreeMap<String, Vec<TrackedEndpoint>>,
    outputs: &mut BTreeMap<String, TrackedEndpoint>,
) -> Result<(), CanonicalExpansionDiagnostic> {
    match (source, sink) {
        (StageSource::Internal(source), StageSink::Internal(sink)) => {
            let connection_track = connection_track(source.track, sink.track)?;
            validate_connection_contract(&source.port, &sink.port, connection_track)?;
            let Endpoint {
                gear_id: source_gear_id,
                port: source_port,
            } = source.endpoint;
            let Endpoint {
                gear_id: sink_gear_id,
                port: sink_port,
            } = sink.endpoint;
            connections.push(CheckedConnection {
                source_gear_id,
                source_port_id: source_port.port_id,
                sink_gear_id,
                sink_port_id: sink_port.port_id,
                value_kind: match connection_track {
                    conduit_core::ConnectionTrack::Payload => source_port.value_kind.clone(),
                    conduit_core::ConnectionTrack::NormalClose => {
                        conduit_core::kind_id(conduit_core::EMPTY_INFO_ID)
                    }
                    conduit_core::ConnectionTrack::Quiescence => {
                        conduit_core::kind_id(conduit_core::EMPTY_INFO_ID)
                    }
                    conduit_core::ConnectionTrack::AbnormalTerminal => source_port
                        .abnormal_kind
                        .clone()
                        .expect("abnormal projection validated its exact Fore kind"),
                },
                track: connection_track,
                temporal: if connection_track == conduit_core::ConnectionTrack::Payload {
                    source_port.temporal
                } else {
                    conduit_core::PortTemporal::Value
                },
            });
        }
        (
            StageSource::FaceInput(name, value_type, temporal, abnormal_kind, track),
            StageSink::Internal(mut sink),
        ) => {
            sink.track = track;
            if track == conduit_core::ConnectionTrack::Payload {
                validate_front_contract(&name, &value_type, temporal, &sink.port, true)?;
            } else {
                let source = conduit_core::PortDescriptor {
                    port_id: conduit_core::port_id(name.as_str()),
                    value_kind: value_type,
                    direction: conduit_core::PortDirection::Output,
                    temporal,
                    abnormal_kind,
                };
                validate_connection_contract(&source, &sink.port, track)?;
            }
            let endpoints = inputs.entry(name.clone()).or_default();
            if endpoints.iter().any(|endpoint| {
                endpoint.gear_id == sink.gear_id && endpoint.port.port_id == sink.port.port_id
            }) {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-047",
                    format!("runtime front input '{name}' repeats one internal binding"),
                ));
            }
            endpoints.push(sink);
        }
        (
            StageSource::Internal(source),
            StageSink::FaceOutput(name, value_type, temporal, abnormal_kind),
        ) => {
            if source.track == conduit_core::ConnectionTrack::Payload {
                validate_front_contract(&name, &value_type, temporal, &source.port, false)?;
            } else {
                let sink = conduit_core::PortDescriptor {
                    port_id: conduit_core::port_id(name.as_str()),
                    value_kind: value_type,
                    direction: conduit_core::PortDirection::Input,
                    temporal,
                    abnormal_kind,
                };
                validate_connection_contract(&source.port, &sink, source.track)?;
            }
            insert_boundary(outputs, name, source)?;
        }
        (StageSource::FaceInput(_, _, _, _, _), StageSink::FaceOutput(_, _, _, _)) => {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "runtime front passthrough must cross an admitted gear".into(),
            ));
        }
    }
    Ok(())
}

fn connection_track(
    source: conduit_core::ConnectionTrack,
    sink: conduit_core::ConnectionTrack,
) -> Result<conduit_core::ConnectionTrack, CanonicalExpansionDiagnostic> {
    use conduit_core::ConnectionTrack;
    match (source, sink) {
        (ConnectionTrack::Payload, track) | (track, ConnectionTrack::Payload) => Ok(track),
        (left, right) if left == right => Ok(left),
        _ => Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-045",
            "one Cord cannot combine normal-close and abnormal-terminal tracks".into(),
        )),
    }
}

fn insert_boundary(
    boundaries: &mut BTreeMap<String, TrackedEndpoint>,
    name: String,
    endpoint: TrackedEndpoint,
) -> Result<(), CanonicalExpansionDiagnostic> {
    if boundaries.insert(name.clone(), endpoint).is_some() {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-047",
            format!("runtime front port '{name}' has multiple internal bindings"),
        ));
    }
    Ok(())
}

pub(super) fn validate_front_bindings(
    plot: &CheckedCanonicalPlot,
    inputs: &BTreeMap<String, Vec<TrackedEndpoint>>,
    outputs: &BTreeMap<String, TrackedEndpoint>,
) -> Result<(), CanonicalExpansionDiagnostic> {
    for port in &plot.runtime_ports {
        let bound = match port.direction {
            RuntimePortDirection::Input => inputs.contains_key(&port.name.text),
            RuntimePortDirection::Output => outputs.contains_key(&port.name.text),
        };
        if !bound {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-048",
                format!(
                    "runtime front port '{}' is not bound exactly once in its back",
                    port.name.text
                ),
            ));
        }
    }
    Ok(())
}
