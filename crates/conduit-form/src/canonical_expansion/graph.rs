use super::*;

pub(super) fn checked_face_ports<'a>(
    form: &'a CheckedCanonicalForm,
    runtime_face: &'a conduit_core::CheckedFace,
) -> BTreeMap<&'a str, (&'a crate::RuntimePort, &'a conduit_core::PortDescriptor)> {
    let descriptors = runtime_face
        .inputs()
        .iter()
        .chain(runtime_face.outputs())
        .map(|port| (port.port_id.as_str(), port))
        .collect::<BTreeMap<_, _>>();
    form.runtime_ports
        .iter()
        .map(|port| {
            let descriptor = descriptors
                .get(port.name.text.as_str())
                .expect("checked runtime face retains every syntax Port");
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
    definition: &crate::KindDefinition,
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
                    parse_configuration_value(&field.key, value, &field.validation)
                })?;
            let accepted = match (&field.validation, &value) {
                (ConfigurationRule::Any, ConfigurationValue::Structured(_)) => false,
                (ConfigurationRule::Any, _) => true,
                (
                    ConfigurationRule::U64Range { minimum, maximum },
                    ConfigurationValue::U64(value),
                ) => (*minimum..=*maximum).contains(value),
                (
                    ConfigurationRule::I64Range { minimum, maximum },
                    ConfigurationValue::I64(value),
                ) => (*minimum..=*maximum).contains(value),
                (
                    ConfigurationRule::DurationMillis { minimum, maximum },
                    ConfigurationValue::U64(value),
                ) => (*minimum..=*maximum).contains(value),
                (ConfigurationRule::TextBytes { maximum }, ConfigurationValue::Text(value)) => {
                    value.len() <= *maximum as usize
                }
                (ConfigurationRule::TextOneOf { values }, ConfigurationValue::Text(value)) => {
                    values.contains(value)
                }
                (
                    ConfigurationRule::Structured { profile },
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
            | CanonicalStartupValue::FormParameter(_)
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
    validation: &ConfigurationRule,
) -> Result<ConfigurationValue, CanonicalExpansionDiagnostic> {
    if let ConfigurationRule::Structured { profile } = validation {
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
    let CanonicalStartupValue::Literal(literal) = value else {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-039",
            format!("startup value '{name}' remains unresolved"),
        ));
    };
    if matches!(validation, ConfigurationRule::DurationMillis { .. }) {
        parse_duration_millis(&literal)
            .map(ConfigurationValue::U64)
            .ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-041",
                    format!("primitive startup duration '{name}' is invalid or overflows"),
                )
            })
    } else if matches!(validation, ConfigurationRule::I64Range { .. }) {
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
    face_ports: &BTreeMap<&str, (&crate::RuntimePort, &conduit_core::PortDescriptor)>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    if let Some((port, descriptor)) = face_ports.get(reference) {
        return Ok(match port.direction {
            RuntimePortDirection::Input => Stage {
                input: None,
                output: Some(StageSource::FaceInput(
                    reference.to_string(),
                    descriptor.value_kind.clone(),
                    crate::value_type::canonical_port_temporal(port.temporal),
                )),
            },
            RuntimePortDirection::Output => Stage {
                input: Some(vec![StageSink::FaceOutput(
                    reference.to_string(),
                    descriptor.value_kind.clone(),
                    crate::value_type::canonical_port_temporal(port.temporal),
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
            format!("cord references unknown gear or face port '{reference}'"),
        )
    })?;
    stage_for_instance(instance_name, instance, explicit_port)
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
                "gear '{instance_name}' has no shorthand face path; name an exact runtime port"
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

pub(super) fn connect(
    source: StageSource,
    sink: StageSink,
    connections: &mut Vec<CheckedConnection>,
    inputs: &mut BTreeMap<String, Vec<Endpoint>>,
    outputs: &mut BTreeMap<String, Endpoint>,
) -> Result<(), CanonicalExpansionDiagnostic> {
    match (source, sink) {
        (StageSource::Internal(source), StageSink::Internal(sink)) => {
            if source.port.value_kind != sink.port.value_kind
                || source.port.temporal != sink.port.temporal
            {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-045",
                    "cord connects incompatible runtime value or temporal contracts".into(),
                ));
            }
            connections.push(CheckedConnection {
                source_gear_id: source.gear_id,
                source_port_id: source.port.port_id,
                sink_gear_id: sink.gear_id,
                sink_port_id: sink.port.port_id,
                value_kind: source.port.value_kind,
                temporal: source.port.temporal,
            });
        }
        (StageSource::FaceInput(name, value_type, temporal), StageSink::Internal(sink)) => {
            require_face_contract(&name, &value_type, temporal, &sink.port)?;
            let endpoints = inputs.entry(name.clone()).or_default();
            if endpoints.iter().any(|endpoint| {
                endpoint.gear_id == sink.gear_id && endpoint.port.port_id == sink.port.port_id
            }) {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-047",
                    format!("runtime face input '{name}' repeats one internal binding"),
                ));
            }
            endpoints.push(sink);
        }
        (StageSource::Internal(source), StageSink::FaceOutput(name, value_type, temporal)) => {
            require_face_contract(&name, &value_type, temporal, &source.port)?;
            insert_boundary(outputs, name, source)?;
        }
        (StageSource::FaceInput(_, _, _), StageSink::FaceOutput(_, _, _)) => {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "runtime face passthrough must cross an admitted gear".into(),
            ));
        }
    }
    Ok(())
}

fn insert_boundary(
    boundaries: &mut BTreeMap<String, Endpoint>,
    name: String,
    endpoint: Endpoint,
) -> Result<(), CanonicalExpansionDiagnostic> {
    if boundaries.insert(name.clone(), endpoint).is_some() {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-047",
            format!("runtime face port '{name}' has multiple internal bindings"),
        ));
    }
    Ok(())
}

fn require_face_contract(
    name: &str,
    value_kind: &conduit_core::KindId,
    temporal: conduit_core::PortTemporal,
    actual: &conduit_core::PortDescriptor,
) -> Result<(), CanonicalExpansionDiagnostic> {
    if value_kind != &actual.value_kind || temporal != actual.temporal {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-045",
            format!(
                "runtime face port '{name}' declares '{}' but binds '{}'",
                value_kind.as_str(),
                actual.value_kind.as_str()
            ),
        ));
    }
    Ok(())
}

pub(super) fn validate_face_bindings(
    form: &CheckedCanonicalForm,
    inputs: &BTreeMap<String, Vec<Endpoint>>,
    outputs: &BTreeMap<String, Endpoint>,
) -> Result<(), CanonicalExpansionDiagnostic> {
    for port in &form.runtime_ports {
        let bound = match port.direction {
            RuntimePortDirection::Input => inputs.contains_key(&port.name.text),
            RuntimePortDirection::Output => outputs.contains_key(&port.name.text),
        };
        if !bound {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-048",
                format!(
                    "runtime face port '{}' is not bound exactly once in its back",
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
