#![no_std]

//! Host-neutral, bounded UTF-8 text Kind semantics.
//!
//! This crate owns text Kind identity, exact typed faces, semantic
//! configuration, finite bounds, and canonical Form catalog installation. It
//! owns no Host implementation, execution profile, host operation, artifact,
//! resource, authority, or manifestation claim.

extern crate alloc;

use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal,
};

pub const TEXT_VALUE_KIND: &str = "value/text@1";
pub const MAX_TEXT_BYTES: u32 = 256;

pub const TEXT_LITERAL_KIND: &str = "text/literal";
pub const TEXT_LITERAL_CONTRACT_REVISION: &str = "conduit.std/text-literal@1";
pub const TEXT_UPPER_KIND: &str = "text/upper";
pub const TEXT_UPPER_CONTRACT_REVISION: &str = "conduit.std/text-upper@1";
pub const TEXT_JOIN_KIND: &str = "text/join";
pub const TEXT_JOIN_CONTRACT_REVISION: &str = "conduit.std/text-join@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextConfigurationField {
    pub key: &'static str,
    pub default_value: ConfigurationValue,
    pub maximum_text_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextKindContract {
    pub kind_id: conduit_core::KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<TextConfigurationField>,
    pub limits: CapabilityLimits,
}

pub fn text_literal_semantics() -> TextKindContract {
    TextKindContract {
        kind_id: kind_id(TEXT_LITERAL_KIND),
        kind_contract_revision: KindContractRevision::from(TEXT_LITERAL_CONTRACT_REVISION),
        inputs: Vec::new(),
        outputs: vec![text_port(PortDirection::Output)],
        configuration: vec![TextConfigurationField {
            key: "value",
            default_value: ConfigurationValue::Text(String::new()),
            maximum_text_bytes: MAX_TEXT_BYTES,
        }],
        limits: text_limits(),
    }
}

pub fn text_upper_semantics() -> TextKindContract {
    TextKindContract {
        kind_id: kind_id(TEXT_UPPER_KIND),
        kind_contract_revision: KindContractRevision::from(TEXT_UPPER_CONTRACT_REVISION),
        inputs: vec![text_port(PortDirection::Input)],
        outputs: vec![text_port(PortDirection::Output)],
        configuration: Vec::new(),
        limits: text_limits(),
    }
}

pub fn text_join_semantics() -> TextKindContract {
    TextKindContract {
        kind_id: kind_id(TEXT_JOIN_KIND),
        kind_contract_revision: KindContractRevision::from(TEXT_JOIN_CONTRACT_REVISION),
        inputs: vec![text_port(PortDirection::Input)],
        outputs: vec![text_port(PortDirection::Output)],
        configuration: vec![TextConfigurationField {
            key: "prefix",
            default_value: ConfigurationValue::Text(String::new()),
            maximum_text_bytes: MAX_TEXT_BYTES,
        }],
        limits: text_limits(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_text_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };

    for (kind, parameter) in [
        (TEXT_LITERAL_KIND, Some(("value", "Text"))),
        (TEXT_UPPER_KIND, None),
        (TEXT_JOIN_KIND, Some(("prefix", "Text"))),
    ] {
        startup.insert(KindSignature {
            kind: kind.to_string(),
            startup_parameters: parameter
                .into_iter()
                .map(|(name, value_type)| StartupParameterSignature {
                    name: name.to_string(),
                    value_type: value_type.to_string(),
                    default: None,
                })
                .collect(),
        })?;
    }
    for contract in [
        text_literal_semantics(),
        text_upper_semantics(),
        text_join_semantics(),
    ] {
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .into_iter()
                    .map(|field| ConfigurationField {
                        key: field.key.to_string(),
                        default_value: field.default_value,
                        validation: ConfigurationRule::TextBytes {
                            maximum: field.maximum_text_bytes,
                        },
                    })
                    .collect(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn text_port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("text"),
        value_kind: kind_id(TEXT_VALUE_KIND),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn text_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 4,
        max_queue_bytes: MAX_TEXT_BYTES,
    }
}
