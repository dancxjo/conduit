use super::*;

pub(super) fn inline_key(cell: &CheckedCanonicalCell) -> String {
    format!("{}:{:?}", cell.operation, cell.startup_bindings)
}

pub(super) fn configuration(
    cell: &CheckedCanonicalCell,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    definition: &crate::KindDefinition,
) -> Result<Vec<conduit_core::ConfigurationEntry>, CanonicalExpansionDiagnostic> {
    if let Some(binding) = cell.startup_bindings.iter().find(|binding| {
        !definition
            .configuration
            .iter()
            .any(|field| field.key == binding.name)
    }) {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            format!(
                "startup parameter '{}' has no exact primitive planning field",
                binding.name
            ),
        ));
    }
    definition
        .configuration
        .iter()
        .map(|field| {
            let value = cell
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
                .and_then(|value| parse_configuration_value(&field.key, value))?;
            let accepted = match (&field.validation, &value) {
                (ConfigurationRule::Any, _) => true,
                (
                    ConfigurationRule::U64Range { minimum, maximum },
                    ConfigurationValue::U64(value),
                ) => (*minimum..=*maximum).contains(value),
                (ConfigurationRule::TextBytes { maximum }, ConfigurationValue::Text(value)) => {
                    value.len() <= *maximum as usize
                }
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

fn parse_configuration_value(
    name: &str,
    value: CanonicalStartupValue,
) -> Result<ConfigurationValue, CanonicalExpansionDiagnostic> {
    let CanonicalStartupValue::Literal(literal) = value else {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-039",
            format!("startup value '{name}' remains unresolved"),
        ));
    };
    if literal == "true" || literal == "false" {
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

pub(super) fn resolve_reference(
    reference: &str,
    instances: &BTreeMap<String, Instance>,
    face_ports: &BTreeMap<&str, &crate::RuntimePort>,
) -> Result<Stage, CanonicalExpansionDiagnostic> {
    if let Some(port) = face_ports.get(reference) {
        return Ok(match port.direction {
            RuntimePortDirection::Input => Stage {
                input: None,
                output: Some(StageSource::FaceInput(
                    reference.to_string(),
                    port.value_type.text.clone(),
                )),
            },
            RuntimePortDirection::Output => Stage {
                input: Some(vec![StageSink::FaceOutput(
                    reference.to_string(),
                    port.value_type.text.clone(),
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
            format!("cord references unknown cell or face port '{reference}'"),
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
                format!("cell '{instance_name}' has no runtime port '{port}'"),
            ));
        }
        return Ok(Stage { input, output });
    }
    let (input, output) = instance.bare_ports.as_ref().ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-044",
            format!(
                "cell '{instance_name}' has no shorthand face path; name an exact runtime port"
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
            if source.port.value_kind != sink.port.value_kind {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-045",
                    "cord connects incompatible runtime value kinds".into(),
                ));
            }
            connections.push(CheckedConnection {
                source_operation_id: source.operation_id,
                source_port_id: source.port.port_id,
                sink_operation_id: sink.operation_id,
                sink_port_id: sink.port.port_id,
                value_kind: source.port.value_kind,
            });
        }
        (StageSource::FaceInput(name, value_type), StageSink::Internal(sink)) => {
            require_face_kind(&name, &value_type, &sink.port.value_kind)?;
            let endpoints = inputs.entry(name.clone()).or_default();
            if endpoints.iter().any(|endpoint| {
                endpoint.operation_id == sink.operation_id
                    && endpoint.port.port_id == sink.port.port_id
            }) {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-047",
                    format!("runtime face input '{name}' repeats one internal binding"),
                ));
            }
            endpoints.push(sink);
        }
        (StageSource::Internal(source), StageSink::FaceOutput(name, value_type)) => {
            require_face_kind(&name, &value_type, &source.port.value_kind)?;
            insert_boundary(outputs, name, source)?;
        }
        (StageSource::FaceInput(_, _), StageSink::FaceOutput(_, _)) => {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "runtime face passthrough must cross an admitted cell".into(),
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

fn require_face_kind(
    name: &str,
    value_type: &str,
    actual: &KindId,
) -> Result<(), CanonicalExpansionDiagnostic> {
    if value_type != actual.as_str() {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-045",
            format!(
                "runtime face port '{name}' declares '{value_type}' but binds '{}'",
                actual.as_str()
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
