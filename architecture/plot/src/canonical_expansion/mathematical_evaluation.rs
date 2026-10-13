//! Seal the authored computation and consumer requirement into one derived Gear.
use super::*;
use conduit_core::{ConfigurationEntry, KindConfigurationField, KindConfigurationRule};

pub(super) fn specialize(
    gear: &CheckedCanonicalGear,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    template: &crate::KindProjection,
) -> Result<Option<(crate::KindProjection, Vec<ConfigurationEntry>)>, CanonicalExpansionDiagnostic>
{
    if gear.kind != "math/evaluate" {
        return Ok(None);
    }
    let (graph, source_identity) = gear
        .numeric_computation
        .as_ref()
        .ok_or_else(|| failure("math/evaluate lost its checked source computation"))?;
    let binding = gear
        .startup_bindings
        .iter()
        .find(|binding| binding.name == "requirement")
        .ok_or_else(|| failure("math/evaluate requires an ordinary typed numeric requirement"))?;
    let rule = KindConfigurationRule::Structured {
        profile: conduit_core::numeric_requirement_profile(),
    };
    let field = KindConfigurationField {
        key: "requirement".into(),
        rule: rule.clone(),
        default_value: ConfigurationValue::Bool(false),
    };
    let requirement =
        validate_startup_configuration(&field, substitute(&binding.value, environment)?)?;
    let ConfigurationValue::Structured(requirement_value) = &requirement else {
        return Err(failure(
            "numeric requirement has an incompatible canonical Type",
        ));
    };
    let checked_requirement =
        conduit_core::NumericRequirement::from_configuration(requirement_value)
            .map_err(|error| failure(&format!("invalid numeric requirement: {error:?}")))?;
    let ieee = graph
        .nodes()
        .iter()
        .any(|node| node.semantic_kind == conduit_core::F64_INFO_ID);
    if ieee != (checked_requirement.semantics == conduit_core::NumericSemantics::IeeeOperational) {
        return Err(failure(
            "numeric requirement semantics do not match the checked computation ontology",
        ));
    }
    let graph_value = graph
        .configuration(*source_identity)
        .map_err(|error| failure(&format!("invalid checked computation: {error:?}")))?;
    let kind = derived_kind(graph, source_identity, requirement_value)?;
    let source = ConfigurationValue::Text(
        source_identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    );
    let mut definition = template.clone();
    definition.kind_id = kind;
    definition.kind_contract_revision = conduit_core::KindIdentity::from("conduit.math/evaluate@1");
    definition.outputs[0].value_kind =
        conduit_core::kind_id(&graph.nodes()[graph.root() as usize].semantic_kind);
    definition.configuration = vec![
        KindConfigurationField {
            key: "numeric_source_identity".into(),
            rule: KindConfigurationRule::TextBytes { maximum: 64 },
            default_value: source.clone(),
        },
        KindConfigurationField {
            key: "numeric_computation".into(),
            rule: KindConfigurationRule::Structured {
                profile: graph_value.profile().clone(),
            },
            default_value: ConfigurationValue::Structured(graph_value.clone()),
        },
        KindConfigurationField {
            key: "numeric_requirement".into(),
            rule,
            default_value: requirement.clone(),
        },
    ];
    let configuration = vec![
        ConfigurationEntry {
            key: "numeric_source_identity".into(),
            value: source,
        },
        ConfigurationEntry {
            key: "numeric_computation".into(),
            value: ConfigurationValue::Structured(graph_value),
        },
        ConfigurationEntry {
            key: "numeric_requirement".into(),
            value: requirement,
        },
    ];
    Ok(Some((definition, configuration)))
}
fn failure(message: &str) -> CanonicalExpansionDiagnostic {
    CanonicalExpansionDiagnostic::new("CND-FRM-053", message.into())
}

fn derived_kind(
    graph: &conduit_core::NumericComputation,
    source: &[u8; 32],
    requirement: &conduit_core::StructuredConfigurationValue,
) -> Result<conduit_core::KindId, CanonicalExpansionDiagnostic> {
    let mut identity = graph
        .canonical_bytes()
        .map_err(|error| failure(&format!("invalid computation: {error:?}")))?;
    identity.extend_from_slice(source);
    identity.extend_from_slice(requirement.canonical_value());
    let digest = conduit_core::semantic_digest("conduit.math.evaluate.consumer.v1", &identity);
    let digest = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(conduit_core::kind_id(&format!(
        "conduitese/math/evaluate/{digest}"
    )))
}

/// Independently check the external source seal and both captured startup capsules.
pub fn checked_gear_numeric_computation(
    gear: &crate::CheckedGear,
) -> Result<conduit_core::NumericComputation, CanonicalExpansionDiagnostic> {
    computation_from_parts(
        &gear.kind_id,
        &gear.kind_contract_revision,
        &gear.inputs,
        &gear.outputs,
        &gear.semantic_contract,
        &gear.configuration,
    )
}
/// Re-admit the same source custody after Planner selected and sealed a Back.
pub fn planned_gear_numeric_computation(
    gear: &conduit_core::PlannedGear,
) -> Result<conduit_core::NumericComputation, CanonicalExpansionDiagnostic> {
    computation_from_parts(
        &gear.kind_id,
        &gear.kind_contract_revision,
        &gear.inputs,
        &gear.outputs,
        &gear.semantic_contract,
        &gear.configuration,
    )
}
fn computation_from_parts(
    kind_id: &conduit_core::KindId,
    revision: &conduit_core::KindIdentity,
    inputs: &[conduit_core::PortDescriptor],
    outputs: &[conduit_core::PortDescriptor],
    contract: &conduit_core::KindSemanticContract,
    configuration: &[ConfigurationEntry],
) -> Result<conduit_core::NumericComputation, CanonicalExpansionDiagnostic> {
    if !kind_id.as_str().starts_with("conduitese/math/evaluate/") {
        return Err(failure("Gear is not a generated mathematical evaluation"));
    }
    if revision.as_str() != "conduit.math/evaluate@1" || !inputs.is_empty() || outputs.len() != 1 {
        return Err(failure(
            "generated numeric Gear has an incompatible contract",
        ));
    }
    for name in [
        "numeric_source_identity",
        "numeric_computation",
        "numeric_requirement",
    ] {
        if configuration
            .iter()
            .filter(|entry| entry.key == name)
            .count()
            != 1
        {
            return Err(failure(
                "numeric Gear startup custody is missing or duplicated",
            ));
        }
        let entry = configuration
            .iter()
            .find(|entry| entry.key == name)
            .expect("checked exact count");
        let default = contract
            .configuration
            .iter()
            .find(|field| field.key == name)
            .ok_or_else(|| failure("numeric Gear semantic contract omits a sealed field"))?;
        if default.default_value != entry.value {
            return Err(failure(
                "numeric Gear startup value diverges from its admitted semantic contract",
            ));
        }
    }
    let field = |name: &str| {
        configuration
            .iter()
            .find(|entry| entry.key == name)
            .map(|entry| &entry.value)
            .ok_or_else(|| failure("numeric Gear is missing a sealed startup field"))
    };
    let ConfigurationValue::Text(source) = field("numeric_source_identity")? else {
        return Err(failure("invalid source identity Type"));
    };
    if source.len() != 64 {
        return Err(failure("invalid source identity extent"));
    }
    let mut identity = [0u8; 32];
    for (index, pair) in source.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let nibble = |byte| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        identity[index] = nibble(pair[0])
            .zip(nibble(pair[1]))
            .map(|(high, low)| high * 16 + low)
            .ok_or_else(|| failure("noncanonical source identity"))?;
    }
    let ConfigurationValue::Structured(graph) = field("numeric_computation")? else {
        return Err(failure("invalid graph Type"));
    };
    let graph = conduit_core::NumericComputation::from_configuration(graph, identity)
        .map_err(|error| failure(&format!("graph source seal mismatch: {error:?}")))?;
    let ConfigurationValue::Structured(requirement) = field("numeric_requirement")? else {
        return Err(failure("invalid requirement Type"));
    };
    conduit_core::NumericRequirement::from_configuration(requirement)
        .map_err(|error| failure(&format!("invalid requirement: {error:?}")))?;
    if derived_kind(&graph, &identity, requirement)? != *kind_id {
        return Err(failure(
            "numeric startup capsules do not match the admitted Kind identity",
        ));
    }
    if outputs
        .first()
        .map(|port| port.value_kind.as_str())
        != Some(graph.nodes()[graph.root() as usize].semantic_kind.as_str())
    {
        return Err(failure(
            "numeric result Type does not match the checked graph",
        ));
    }
    Ok(graph)
}
