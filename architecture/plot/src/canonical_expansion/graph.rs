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
                .and_then(|value| {
                    parse_configuration_value(&field.key, value, &field.rule)
                })?;
            let accepted = match (&field.rule, &value) {
                (KindConfigurationRule::Any, ConfigurationValue::Structured(_)) => false,
                (KindConfigurationRule::Any, _) => true,
                (
                    KindConfigurationRule::U64Range { minimum, maximum },
                    ConfigurationValue::U64(value),
                ) => (*minimum..=*maximum).contains(value),
                (
                    KindConfigurationRule::I64Range { minimum, maximum },
                    ConfigurationValue::I64(value),
                ) => (*minimum..=*maximum).contains(value),
                (
                    KindConfigurationRule::DurationMillis { minimum, maximum },
                    ConfigurationValue::U64(value),
                ) => (*minimum..=*maximum).contains(value),
                (
                    KindConfigurationRule::QuantityRange {
                        minimum,
                        maximum,
                        canonical_unit,
                    },
                    ConfigurationValue::Quantity(value),
                ) => value
                    .convert(*canonical_unit)
                    .is_ok_and(|value| (*minimum..=*maximum).contains(&value.value())),
                (KindConfigurationRule::TextBytes { maximum }, ConfigurationValue::Text(value)) => {
                    value.len() <= *maximum as usize
                }
                (KindConfigurationRule::TextOneOf { values }, ConfigurationValue::Text(value)) => {
                    values.contains(value)
                }
                (
                    KindConfigurationRule::Structured { profile },
                    ConfigurationValue::Structured(value),
                ) => value.profile() == profile,
                _ => false,
            };
            if !accepted {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-040",
                    format!(
                        "startup value for '{}' violates its primitive contract",
                        field.key
                    ),
                ));
            }
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
            | CanonicalStartupValue::PlotParameter(_)
            | CanonicalStartupValue::Structured(_) => None,
        })
        .collect::<Vec<_>>();
    pools.sort();
    pools.dedup();
    Ok(pools)
}

fn parse_configuration_value(
    name: &str,
    value: CanonicalStartupValue,
    rule: &KindConfigurationRule,
) -> Result<ConfigurationValue, CanonicalExpansionDiagnostic> {
    if let KindConfigurationRule::Structured { profile } = rule {
        let CanonicalStartupValue::Structured(value) = value else {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-039",
                format!("structured startup value '{name}' remains unresolved"),
            ));
        };
        let actual_profile = value.value_type().profile().map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                format!("structured startup value '{name}' has no finite profile"),
            )
        })?;
        if actual_profile.value_kind() != profile {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                format!("structured startup value '{name}' violates its exact profile"),
            ));
        }
        let concrete = value.try_concrete().ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-039",
                format!("structured startup value '{name}' remains unresolved"),
            )
        })?;
        let canonical = concrete.canonical_bytes().map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                format!("structured startup value '{name}' exceeds canonical bounds"),
            )
        })?;
        let structured =
            conduit_core::StructuredConfigurationValue::new(profile.clone(), canonical)
                .ok_or_else(|| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-041",
                        format!("structured startup value '{name}' exceeds configuration bounds"),
                    )
                })?;
        return Ok(ConfigurationValue::Structured(structured));
    }
    if matches!(rule, KindConfigurationRule::QuantityRange { .. }) {
        return match value {
            CanonicalStartupValue::Quantity(quantity) => Ok(ConfigurationValue::Quantity(quantity)),
            CanonicalStartupValue::Literal(literal) => {
                conduit_core::Quantity::parse_plot_literal(&literal)
                    .map(ConfigurationValue::Quantity)
                    .map_err(|_| {
                        CanonicalExpansionDiagnostic::new(
                            "CND-FRM-041",
                            format!("primitive startup quantity '{name}' is invalid"),
                        )
                    })
            }
            _ => Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-039",
                format!("startup value '{name}' remains unresolved"),
            )),
        };
    }
    if matches!(rule, KindConfigurationRule::DurationMillis { .. }) {
        if let CanonicalStartupValue::Quantity(quantity) = value {
            let milliseconds = quantity
                .convert(conduit_core::QuantityUnit::Millisecond)
                .map_err(|_| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-041",
                        format!("primitive startup duration '{name}' is invalid or inexact"),
                    )
                })?;
            return u64::try_from(milliseconds.value())
                .map(ConfigurationValue::U64)
                .map_err(|_| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-041",
                        format!("primitive startup duration '{name}' is negative or overflows"),
                    )
                });
        }
    }
    let CanonicalStartupValue::Literal(literal) = value else {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-039",
            format!("startup value '{name}' remains unresolved"),
        ));
    };
    if matches!(rule, KindConfigurationRule::DurationMillis { .. }) {
        parse_duration_millis(&literal)
            .map(ConfigurationValue::U64)
            .ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-041",
                    format!("primitive startup duration '{name}' is invalid or overflows"),
                )
            })
    } else if matches!(rule, KindConfigurationRule::I64Range { .. }) {
        parse_scalar_configuration(&literal)
            .map(ConfigurationValue::I64)
            .ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-041",
                    format!("primitive startup scalar '{name}' is invalid or overflows"),
                )
            })
    } else if literal == "true" || literal == "false" {
        Ok(ConfigurationValue::Bool(literal == "true"))
    } else if let Ok(value) = literal.parse::<u64>() {
        Ok(ConfigurationValue::U64(value))
    } else if let Some(value) = crate::text_value::parse_quoted_text(&literal) {
        Ok(ConfigurationValue::Text(value))
    } else {
        Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            format!("primitive startup value '{name}' cannot be represented by the current planner contract"),
        ))
    }
}

fn parse_scalar_configuration(literal: &str) -> Option<i64> {
    if !literal.contains('.') {
        return literal.parse().ok();
    }
    let (negative, magnitude) = literal
        .strip_prefix('-')
        .map_or((false, literal), |value| (true, value));
    let (whole, fraction) = magnitude.split_once('.')?;
    if whole.is_empty()
        || fraction.is_empty()
        || fraction.len() > 6
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let whole = whole.parse::<u64>().ok()?;
    let fraction_digits = fraction.len();
    let fraction = fraction.parse::<u64>().ok()?;
    let magnitude = whole
        .checked_mul(conduit_core::Scalar::SCALE as u64)?
        .checked_add(fraction.checked_mul(10_u64.pow((6 - fraction_digits) as u32))?)?;
    if negative {
        if magnitude == i64::MAX as u64 + 1 {
            Some(i64::MIN)
        } else {
            i64::try_from(magnitude).ok()?.checked_neg()
        }
    } else {
        i64::try_from(magnitude).ok()
    }
}

fn parse_duration_millis(literal: &str) -> Option<u64> {
    let (digits, multiplier) = literal
        .strip_suffix("ms")
        .map(|digits| (digits, 1))
        .or_else(|| literal.strip_suffix('s').map(|digits| (digits, 1_000)))?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u64>().ok()?.checked_mul(multiplier)
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
                        conduit_core::kind_id(conduit_core::UNIT_INFO_ID)
                    }
                    conduit_core::ConnectionTrack::Quiescence => {
                        conduit_core::kind_id(conduit_core::UNIT_INFO_ID)
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
                require_front_contract(&name, &value_type, temporal, &sink.port, true)?;
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
                require_front_contract(&name, &value_type, temporal, &source.port, false)?;
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

fn validate_connection_contract(
    source: &conduit_core::PortDescriptor,
    sink: &conduit_core::PortDescriptor,
    track: conduit_core::ConnectionTrack,
) -> Result<(), CanonicalExpansionDiagnostic> {
    use conduit_core::{ConnectionTrack, PortTemporal};
    let compatible = match track {
        ConnectionTrack::Payload => {
            let reactively_lifted_value = matches!(
                (source.temporal, sink.temporal),
                (PortTemporal::Flow { .. }, PortTemporal::Value)
            );
            let finite_flow_into_standing_consumer = matches!(
                (source.temporal, sink.temporal),
                (
                    PortTemporal::Flow { closes: true },
                    PortTemporal::Flow { closes: false }
                )
            );
            source.value_kind == sink.value_kind
                && (source.temporal == sink.temporal
                    || reactively_lifted_value
                    || finite_flow_into_standing_consumer)
        }
        ConnectionTrack::NormalClose => {
            matches!(source.temporal, PortTemporal::Flow { closes: true })
                && sink.temporal == PortTemporal::Value
                && sink.value_kind.as_str() == conduit_core::UNIT_INFO_ID
        }
        ConnectionTrack::Quiescence => {
            matches!(source.temporal, PortTemporal::Flow { .. })
                && sink.temporal == PortTemporal::Value
                && sink.value_kind.as_str() == conduit_core::UNIT_INFO_ID
        }
        ConnectionTrack::AbnormalTerminal => {
            sink.temporal == PortTemporal::Value
                && source.abnormal_kind.as_ref() == Some(&sink.value_kind)
        }
    };
    if compatible {
        return Ok(());
    }
    Err(CanonicalExpansionDiagnostic::new(
        "CND-FRM-045",
        format!(
            "cord connects incompatible {} contracts: source {} {} -> sink {} {}",
            track.as_str(),
            source.value_kind.as_str(),
            source.temporal.as_str(),
            sink.value_kind.as_str(),
            sink.temporal.as_str()
        ),
    ))
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

fn require_front_contract(
    name: &str,
    value_kind: &conduit_core::KindId,
    temporal: conduit_core::PortTemporal,
    actual: &conduit_core::PortDescriptor,
    front_is_source: bool,
) -> Result<(), CanonicalExpansionDiagnostic> {
    let reactive_flow_boundary = matches!(
        (temporal, actual.temporal),
        (
            conduit_core::PortTemporal::Flow { .. },
            conduit_core::PortTemporal::Value
        ) | (
            conduit_core::PortTemporal::Value,
            conduit_core::PortTemporal::Flow { .. }
        )
    );
    let finite_flow_into_standing_consumer = front_is_source
        && matches!(
            (temporal, actual.temporal),
            (
                conduit_core::PortTemporal::Flow { closes: true },
                conduit_core::PortTemporal::Flow { closes: false }
            )
        );
    if value_kind != &actual.value_kind
        || (temporal != actual.temporal
            && !reactive_flow_boundary
            && !finite_flow_into_standing_consumer)
    {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-045",
            format!(
                "runtime front port '{name}' declares '{}' ({temporal:?}) but binds '{}' ({:?})",
                value_kind.as_str(),
                actual.value_kind.as_str(),
                actual.temporal,
            ),
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

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
