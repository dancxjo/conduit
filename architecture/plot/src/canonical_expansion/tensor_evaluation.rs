//! Source custody for a finite collection of ordered TensorF64 coordinate graphs.
use super::*;
use conduit_core::{
    ConfigurationEntry, KindConfigurationField, KindConfigurationRule, NumericComputation,
    NumericRequirement, StructuredConfigurationValue,
};

pub struct CheckedTensorComputations {
    pub coordinates: Vec<NumericComputation>,
    pub requirement: NumericRequirement,
    pub dimensions: StructuredConfigurationValue,
    pub axes: StructuredConfigurationValue,
}
fn failure(message: &str) -> CanonicalExpansionDiagnostic {
    CanonicalExpansionDiagnostic::new("CND-FRM-053", message.into())
}
pub(super) fn specialize(
    gear: &CheckedCanonicalGear,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    template: &crate::KindProjection,
) -> Result<Option<(crate::KindProjection, Vec<ConfigurationEntry>)>, CanonicalExpansionDiagnostic>
{
    if gear.kind != "tensor/evaluate" {
        return Ok(None);
    }
    let graphs = gear
        .tensor_computations
        .as_ref()
        .ok_or_else(|| failure("Tensor lost its ordered source coordinates"))?;
    let mut configuration = Vec::new();
    for name in ["dimensions", "axes", "requirement"] {
        let binding = gear
            .startup_bindings
            .iter()
            .find(|binding| binding.name == name)
            .ok_or_else(|| failure("Tensor missing a typed startup contract"))?;
        let CanonicalStartupValue::Structured(value) = substitute(&binding.value, environment)?
        else {
            return Err(failure("Tensor requires ordinary typed startup values"));
        };
        let concrete = value
            .try_concrete()
            .ok_or_else(|| failure("Tensor startup value is not closed"))?;
        let profile = concrete
            .value_type()
            .profile()
            .map_err(|_| failure("Tensor startup profile is invalid"))?
            .value_kind()
            .clone();
        let encoded = concrete
            .canonical_bytes()
            .map_err(|_| failure("Tensor startup encoding exceeds its bound"))?;
        let value = StructuredConfigurationValue::new(profile, encoded)
            .ok_or_else(|| failure("Tensor startup is noncanonical"))?;
        if name == "requirement" {
            let req = NumericRequirement::from_configuration(&value)
                .map_err(|_| failure("Tensor numerical requirement is invalid"))?;
            if req.semantics != conduit_core::NumericSemantics::Mathematical {
                return Err(failure(
                    "Mathematical Tensor requires mathematical semantics",
                ));
            }
        }
        configuration.push(ConfigurationEntry {
            key: format!("tensor_{name}"),
            value: ConfigurationValue::Structured(value),
        });
    }
    for (index, (graph, source)) in graphs.iter().enumerate() {
        configuration.push(ConfigurationEntry {
            key: format!("tensor_coordinate_{index}"),
            value: ConfigurationValue::Structured(
                graph
                    .configuration(*source)
                    .map_err(|_| failure("Tensor graph exceeds its canonical bound"))?,
            ),
        });
        configuration.push(ConfigurationEntry {
            key: format!("tensor_source_{index}"),
            value: ConfigurationValue::Text(
                source.iter().map(|byte| format!("{byte:02x}")).collect(),
            ),
        });
    }
    let mut definition = template.clone();
    definition.kind_id = derived_kind(&configuration)?;
    definition.kind_contract_revision =
        conduit_core::KindIdentity::from("conduit.tensor/evaluate@1");
    definition.configuration = configuration
        .iter()
        .map(|entry| KindConfigurationField {
            key: entry.key.clone(),
            rule: match &entry.value {
                ConfigurationValue::Structured(value) => KindConfigurationRule::Structured {
                    profile: value.profile().clone(),
                },
                ConfigurationValue::Text(_) => KindConfigurationRule::TextBytes { maximum: 64 },
                _ => unreachable!("sealed Tensor startup inventory"),
            },
            default_value: entry.value.clone(),
        })
        .collect();
    Ok(Some((definition, configuration)))
}
fn derived_kind(
    configuration: &[ConfigurationEntry],
) -> Result<conduit_core::KindId, CanonicalExpansionDiagnostic> {
    let mut bytes = Vec::new();
    for entry in configuration {
        bytes.extend_from_slice(&(entry.key.len() as u32).to_be_bytes());
        bytes.extend_from_slice(entry.key.as_bytes());
        let value = match &entry.value {
            ConfigurationValue::Structured(value) => value.canonical_value(),
            ConfigurationValue::Text(value) => value.as_bytes(),
            _ => return Err(failure("Invalid Tensor startup capsule")),
        };
        bytes.extend_from_slice(&(value.len() as u32).to_be_bytes());
        bytes.extend_from_slice(value);
    }
    let digest = conduit_core::semantic_digest("conduit.tensor.evaluate.source@1", &bytes);
    Ok(conduit_core::kind_id(&format!(
        "conduitese/tensor/evaluate/{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )))
}
pub fn checked_gear_tensor_computations(
    gear: &crate::CheckedGear,
) -> Result<CheckedTensorComputations, CanonicalExpansionDiagnostic> {
    readmit(
        &gear.kind_id,
        &gear.kind_contract_revision,
        &gear.inputs,
        &gear.outputs,
        &gear.semantic_contract,
        &gear.configuration,
    )
}
pub fn planned_gear_tensor_computations(
    gear: &conduit_core::PlannedGear,
) -> Result<CheckedTensorComputations, CanonicalExpansionDiagnostic> {
    readmit(
        &gear.kind_id,
        &gear.kind_contract_revision,
        &gear.inputs,
        &gear.outputs,
        &gear.semantic_contract,
        &gear.configuration,
    )
}
fn readmit(
    kind: &conduit_core::KindId,
    revision: &conduit_core::KindIdentity,
    inputs: &[conduit_core::PortDescriptor],
    outputs: &[conduit_core::PortDescriptor],
    contract: &conduit_core::KindSemanticContract,
    configuration: &[ConfigurationEntry],
) -> Result<CheckedTensorComputations, CanonicalExpansionDiagnostic> {
    if !kind.as_str().starts_with("conduitese/tensor/evaluate/")
        || revision.as_str() != "conduit.tensor/evaluate@1"
        || !inputs.is_empty()
        || outputs.len() != 1
        || outputs[0].value_kind.as_str() != "data/tensor@1"
        || derived_kind(configuration)? != *kind
    {
        return Err(failure("Tensor derived contract/source identity mismatch"));
    }
    if configuration.len() < 5
        || configuration.len() > 19
        || !(configuration.len() - 3).is_multiple_of(2)
    {
        return Err(failure("Tensor coordinate inventory is invalid"));
    }
    let field = |name: &str| -> Result<&ConfigurationValue, CanonicalExpansionDiagnostic> {
        let mut entries = configuration.iter().filter(|entry| entry.key == name);
        let entry = entries
            .next()
            .ok_or_else(|| failure("Tensor startup capsule missing"))?;
        if entries.next().is_some()
            || contract
                .configuration
                .iter()
                .filter(|value| value.key == name && value.default_value == entry.value)
                .count()
                != 1
        {
            return Err(failure(
                "Tensor startup capsule diverged from checked contract",
            ));
        }
        Ok(&entry.value)
    };
    let structured =
        |name: &str| -> Result<StructuredConfigurationValue, CanonicalExpansionDiagnostic> {
            let ConfigurationValue::Structured(value) = field(name)? else {
                return Err(failure("Tensor structured startup Type mismatch"));
            };
            Ok(value.clone())
        };
    let dimensions = structured("tensor_dimensions")?;
    let axes = structured("tensor_axes")?;
    let requirement = NumericRequirement::from_configuration(&structured("tensor_requirement")?)
        .map_err(|_| failure("Tensor numerical requirement is invalid"))?;
    if requirement.semantics != conduit_core::NumericSemantics::Mathematical {
        return Err(failure("Tensor numerical semantics mismatch"));
    }
    let mut coordinates = Vec::new();
    for index in 0..(configuration.len() - 3) / 2 {
        let ConfigurationValue::Text(source) = field(&format!("tensor_source_{index}"))? else {
            return Err(failure("Tensor source seal Type mismatch"));
        };
        if source.len() != 64 {
            return Err(failure("Tensor source seal extent mismatch"));
        }
        let mut identity = [0u8; 32];
        for (position, pair) in source.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let nibble = |byte| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                _ => None,
            };
            identity[position] = nibble(pair[0])
                .zip(nibble(pair[1]))
                .map(|(high, low)| high * 16 + low)
                .ok_or_else(|| failure("Tensor source seal noncanonical"))?;
        }
        let graph = NumericComputation::from_configuration(
            &structured(&format!("tensor_coordinate_{index}"))?,
            identity,
        )
        .map_err(|_| failure("Tensor ordered graph/source seal mismatch"))?;
        if graph.nodes().iter().any(|node| {
            matches!(
                node.operation,
                conduit_core::NumericComputationOperation::Parameter(_)
            )
        }) {
            return Err(failure("Tensor coordinate graph is not closed"));
        }
        let root = &graph.nodes()[graph.root() as usize];
        if root.semantic_kind != conduit_core::MATH_VALUE_INFO_ID
            && conduit_core::PortableNumericProfile::from_info_id(&root.semantic_kind).is_none()
        {
            return Err(failure("Tensor coordinate is not mathematical Scalar"));
        }
        coordinates.push(graph);
    }
    Ok(CheckedTensorComputations {
        coordinates,
        requirement,
        dimensions,
        axes,
    })
}
