//! Exact fixed-image configuration, with separately admitted expression metadata.
use crate::{
    EmbeddedComputationBounds, GeneratedConfigurationEntry, GeneratedConfigurationValue,
    GenerationError, UnsupportedPlanFeature,
};
use conduit_core::{ConfigurationValue, KindSemanticContract, PlanFragment, PlannedGear};
use conduit_plot::{portable_expression_definition, PortableExpressionProgram};

pub(crate) fn generate_configuration(
    fragment: &PlanFragment,
    expressions: Option<EmbeddedComputationBounds>,
) -> Result<Vec<GeneratedConfigurationEntry>, GenerationError> {
    let mut generated = Vec::new();
    let mut total = 0_u64;
    for (index, placement) in fragment.placements.iter().enumerate() {
        let node = u16::try_from(index)
            .map_err(|_| GenerationError::ArithmeticOverflow("configuration node ordinal"))?;
        for entry in &placement.configuration {
            let value = match &entry.value {
                ConfigurationValue::Bool(value) => GeneratedConfigurationValue::Bool(*value),
                ConfigurationValue::I64(value) => GeneratedConfigurationValue::I64(*value),
                ConfigurationValue::U64(value) => GeneratedConfigurationValue::U64(*value),
                ConfigurationValue::Text(value)
                    if entry.key == "label" && conduit_core::valid_data_label(value) =>
                {
                    GeneratedConfigurationValue::Text(value.clone())
                }
                ConfigurationValue::Text(value)
                    if entry.key == "reference" && valid_snapshot_reference(placement, value) =>
                {
                    GeneratedConfigurationValue::Text(value.clone())
                }
                ConfigurationValue::Text(value)
                    if matches!(entry.key.as_str(), "program" | "selector")
                        && expressions.is_some() =>
                {
                    let bounds = expressions.expect("selected expression profile");
                    let size = u64::try_from(value.len() / 2).map_err(|_| {
                        GenerationError::ArithmeticOverflow("expression program extent")
                    })?;
                    bound(
                        "computation program bytes",
                        size,
                        bounds.maximum_program_bytes,
                    )?;
                    total = total
                        .checked_add(size)
                        .ok_or(GenerationError::ArithmeticOverflow(
                            "expression total bytes",
                        ))?;
                    bound(
                        "computation total program bytes",
                        total,
                        bounds.maximum_total_program_bytes,
                    )?;
                    validate_expression(placement, &entry.key, value)?;
                    GeneratedConfigurationValue::Text(value.clone())
                }
                ConfigurationValue::Text(_) => {
                    return Err(GenerationError::Unsupported(
                        UnsupportedPlanFeature::TextConfiguration,
                    ))
                }
                ConfigurationValue::Structured(_) => {
                    return Err(GenerationError::Unsupported(
                        UnsupportedPlanFeature::StructuredConfiguration,
                    ))
                }
                ConfigurationValue::Quantity(_)
                | ConfigurationValue::Unit(_)
                | ConfigurationValue::TemperatureDifference(_) => {
                    return Err(GenerationError::Unsupported(
                        UnsupportedPlanFeature::QuantityConfiguration,
                    ))
                }
            };
            generated.push(GeneratedConfigurationEntry {
                node,
                key: entry.key.clone(),
                value,
            });
        }
    }
    Ok(generated)
}
fn bound(table: &'static str, actual: u64, maximum: u32) -> Result<(), GenerationError> {
    if actual > u64::from(maximum) {
        return Err(GenerationError::BoundExceeded {
            table,
            actual,
            maximum: u64::from(maximum),
        });
    }
    Ok(())
}
pub(crate) fn validate_expression(
    placement: &PlannedGear,
    key: &str,
    text: &str,
) -> Result<(), GenerationError> {
    use GenerationError::InvalidComputationConfiguration as Invalid;
    if placement.configuration.len() != 1
        || placement.inputs.len() != 1
        || placement.outputs.len() != 1
    {
        return Err(Invalid);
    }
    let (definition, laws) = if key == "program" {
        let program = PortableExpressionProgram::from_canonical_hex(text).map_err(|_| Invalid)?;
        (
            portable_expression_definition(&program, placement.inputs[0].temporal)
                .map_err(|_| Invalid)?,
            program.semantic_laws().map_err(|_| Invalid)?,
        )
    } else {
        let selector =
            conduit_core::StructuredSelector::from_canonical_hex(text).map_err(|_| Invalid)?;
        (
            conduit_plot::structured_selector_definition(&selector, placement.inputs[0].temporal),
            conduit_plot::structured_selector_semantic_laws(&selector).map_err(|_| Invalid)?,
        )
    };
    let contract = KindSemanticContract {
        configuration: definition.configuration.clone(),
        laws,
    };
    if placement.kind_id != definition.kind_id
        || placement.kind_contract_revision != definition.kind_contract_revision
        || placement.inputs != definition.inputs
        || placement.outputs != definition.outputs
        || placement.semantic_contract != contract
    {
        return Err(Invalid);
    }
    Ok(())
}

// This carries a checked portable reference, never a local handle or grant.
fn valid_snapshot_reference(placement: &PlannedGear, text: &str) -> bool {
    use conduit_semantic_catalog::{
        resource_snapshot_semantic_contract, SNAPSHOT_PUBLISH_KIND, SNAPSHOT_READ_KIND,
    };
    let publish = match placement.kind_id.as_str() {
        SNAPSHOT_PUBLISH_KIND => true,
        SNAPSHOT_READ_KIND => false,
        _ => return false,
    };
    let kind = resource_snapshot_semantic_contract(publish);
    if placement.configuration.len() != 1
        || placement.kind_contract_revision != kind.kind_contract_revision
        || placement.inputs != kind.inputs
        || placement.outputs != kind.outputs
        || placement.semantic_contract.configuration != kind.configuration
        || placement.semantic_contract.laws != kind.semantic_laws
        || text.len() > 2 * conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES
        || !text.len().is_multiple_of(2)
    {
        return false;
    }
    let mut bytes = [0; conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES];
    for (out, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let digit = |b| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        };
        let (Some(hi), Some(lo)) = (digit(pair[0]), digit(pair[1])) else {
            return false;
        };
        *out = (hi << 4) | lo;
    }
    conduit_core::BoundedResourceRef::validate_encoded(&bytes[..text.len() / 2]).is_ok()
}

#[cfg(test)]
#[path = "snapshot_configuration_tests.rs"]
mod snapshot_tests;
