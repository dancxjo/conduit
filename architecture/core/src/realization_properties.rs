//! Finite typed domain facts of one concrete realization, never catalog membership.
use crate::{push_string, push_u32, StructuredConfigurationValue};
use alloc::vec::Vec;

pub const MAXIMUM_REALIZATION_PROPERTIES: usize = 8;
pub const MAXIMUM_REALIZATION_PROPERTY_BYTES: usize = 32_768;

pub fn valid_realization_properties(properties: &[StructuredConfigurationValue]) -> bool {
    properties.len() <= MAXIMUM_REALIZATION_PROPERTIES
        && properties
            .iter()
            .all(|p| p.canonical_value().len() <= MAXIMUM_REALIZATION_PROPERTY_BYTES)
        && properties
            .windows(2)
            .all(|pair| pair[0].profile() < pair[1].profile())
}

pub(super) fn push_canonical(canonical: &mut Vec<u8>, properties: &[StructuredConfigurationValue]) {
    push_string(canonical, "realization-properties@1");
    push_u32(canonical, properties.len() as u32);
    for property in properties {
        push_string(canonical, property.profile().as_str());
        push_u32(canonical, property.canonical_value().len() as u32);
        canonical.extend_from_slice(property.canonical_value());
    }
}

pub(super) fn validate_kind_requirements(
    kind: &crate::Kind,
) -> Result<(), crate::KindValidationError> {
    use alloc::collections::BTreeSet;
    let mut fields = BTreeSet::new();
    for law in &kind.semantic_laws {
        let crate::KindSemanticLaw::RealizationRequirement {
            property_profile,
            configuration_key,
        } = law
        else {
            continue;
        };
        if property_profile.as_str().is_empty()
            || configuration_key.is_empty()
            || !fields.insert(configuration_key.as_str())
            || !kind.configuration.iter().any(|field| {
                field.key == *configuration_key
                    && matches!(field.rule, crate::KindConfigurationRule::Structured { .. })
            })
        {
            return Err(crate::KindValidationError::InvalidRealizationRequirement);
        }
    }
    if fields.len() > MAXIMUM_REALIZATION_PROPERTIES {
        return Err(crate::KindValidationError::InvalidRealizationRequirement);
    }
    Ok(())
}
