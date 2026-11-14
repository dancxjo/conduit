//! One immutable, Host-installed typed datum, selected by a logical key.
//! This declares meaning, not installed availability, material or authority.
use alloc::{vec, vec::Vec};
use conduit_core::*;

pub const INSTALLED_VALUE_ONCE_KIND: &str = "value/installed/once";
pub const INSTALLED_VALUE_ONCE_REVISION: &str = "value/installed/once@1";
pub const INSTALLED_VALUE_KEY_MAXIMUM_BYTES: u32 = 128;

/// Emit exactly one independently installed value, then close. Neither the
/// value nor a physical address is a Source configuration/default. The Host
/// must separately validate its immutable material against the checked Native
/// laws and bind the exact initialized producer before admitting execution.
pub fn installed_value_once_semantic_contract(
    contract: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    contract
        .validate_definition()
        .map_err(|_| "installed value contract")?;
    if contract.maximum_bytes == 0
        || contract.maximum_bytes as usize > MAXIMUM_STRUCTURED_CANONICAL_BYTES
        || matches!(schema.shape(), StructuredInfoTypeShape::Leaf(_))
        || schema
            .profile()
            .map_err(|_| "installed value schema")?
            .value_kind()
            != &contract.value_kind
    {
        return Err("installed value requires an exact finite structured envelope");
    }
    let key_contract = CheckedValueContract::new(
        kind_id("value/text"),
        INSTALLED_VALUE_KEY_MAXIMUM_BYTES,
        vec![ValueConstraint::ByteLength {
            minimum: 1,
            maximum: INSTALLED_VALUE_KEY_MAXIMUM_BYTES,
        }],
    )
    .map_err(|_| "installed value key contract")?;
    let kind = Kind {
        kind_id: typed_kind_instance_id(
            INSTALLED_VALUE_ONCE_KIND,
            core::iter::once((schema, contract)),
        )
        .map_err(|_| "installed value identity")?,
        kind_contract_revision: INSTALLED_VALUE_ONCE_REVISION.into(),
        startup_parameters: vec![FrontStartupParameter {
            name: "key".into(),
            value_type: kind_id("value/text"),
            has_default: false,
        }],
        shorthand: None,
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: contract.value_kind.clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        // Core's configuration field has a fallback representation even for a
        // required startup argument. Empty is deliberately not a usable key;
        // Source must supply it and selection must call the exact parser below.
        configuration: vec![KindConfigurationField {
            key: "key".into(),
            default_value: ConfigurationValue::Text(alloc::string::String::new()),
            rule: KindConfigurationRule::TextBytes {
                maximum: INSTALLED_VALUE_KEY_MAXIMUM_BYTES,
            },
        }],
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![
            FrontValueContract {
                location: FrontValueLocation::Startup("key".into()),
                contract: key_contract,
            },
            FrontValueContract {
                location: FrontValueLocation::Output(port_id("value")),
                contract: contract.clone(),
            },
        ])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: contract.maximum_bytes,
        },
    };
    kind.validate().map_err(|_| "installed value Kind")?;
    Ok(kind)
}

/// Selection checks the required key; no ambient lookup or fallback is allowed.
pub fn installed_value_once_key(
    configuration: &[ConfigurationEntry],
) -> Result<&str, &'static str> {
    let [entry] = configuration else {
        return Err("installed value requires one logical key");
    };
    let ConfigurationValue::Text(key) = &entry.value else {
        return Err("installed value key must be text");
    };
    if entry.key != "key"
        || key.is_empty()
        || key.len() > INSTALLED_VALUE_KEY_MAXIMUM_BYTES as usize
    {
        return Err("installed value key bound");
    }
    Ok(key)
}

#[cfg(feature = "plot-catalog")]
pub fn install_installed_value_once_kind(
    contract: &CheckedValueContract,
    schema: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = installed_value_once_semantic_contract(contract, schema)
        .map_err(alloc::string::String::from)?;
    let mut staged_startup = startup.clone();
    let mut staged_profiles = profiles.clone();
    staged_startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![conduit_plot::StartupParameterSignature {
            name: "key".into(),
            value_type: "Text".into(),
            default: None,
        }],
    })?;
    staged_startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    staged_startup.insert_structured_type(
        alloc::format!("{}/value", kind.kind_id.as_str()),
        schema.clone(),
    )?;
    staged_profiles
        .insert_kind(kind)
        .map_err(|e| alloc::format!("{e:?}"))?;
    *startup = staged_startup;
    *profiles = staged_profiles;
    Ok(())
}

#[cfg(test)]
#[path = "installed_value_tests.rs"]
mod tests;
