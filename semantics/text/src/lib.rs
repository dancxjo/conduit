#![no_std]

//! Host-neutral, bounded UTF-8 text Kind semantics.
//!
//! This crate owns text Kind identity, exact typed faces, semantic
//! configuration, finite bounds, and canonical Form catalog installation. It
//! owns no Host implementation, execution profile, host operation, artifact,
//! resource, authority, or manifestation claim.

extern crate alloc;

mod addressed_utterance;
mod morse;
#[cfg(feature = "form-catalog")]
mod morse_backs;
mod morse_catalog;
mod morse_key;
mod morse_table;
mod morse_values;
pub use addressed_utterance::*;
pub use morse::*;
#[cfg(feature = "form-catalog")]
pub use morse_backs::*;
pub use morse_catalog::*;
pub use morse_key::*;
pub use morse_values::*;

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
pub const ADDRESS_DETECT_KIND: &str = "text/address-detect";
pub const ADDRESS_DETECT_CONTRACT_REVISION: &str = "conduit.text/address-detect@1";
pub const ADDRESS_SET_VALUE_KIND: &str = "text/address-set@1";
pub const ADDRESS_DETECTION_VALUE_KIND: &str = "text/address-detection@1";

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

pub fn address_detect_semantics() -> TextKindContract {
    TextKindContract {
        kind_id: kind_id(ADDRESS_DETECT_KIND),
        kind_contract_revision: KindContractRevision::from(ADDRESS_DETECT_CONTRACT_REVISION),
        inputs: vec![
            named_text_port("recognized", TEXT_VALUE_KIND, PortDirection::Input),
            named_text_port("addresses", ADDRESS_SET_VALUE_KIND, PortDirection::Input),
        ],
        outputs: vec![named_text_port(
            "detection",
            ADDRESS_DETECTION_VALUE_KIND,
            PortDirection::Output,
        )],
        configuration: Vec::new(),
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

    startup.insert_value_kind_alias("AddressSet", kind_id(ADDRESS_SET_VALUE_KIND))?;
    startup.insert_value_kind_alias("AddressDetection", kind_id(ADDRESS_DETECTION_VALUE_KIND))?;

    for (kind, parameter) in [
        (TEXT_LITERAL_KIND, Some(("value", "Text"))),
        (TEXT_UPPER_KIND, None),
        (TEXT_JOIN_KIND, Some(("prefix", "Text"))),
        (ADDRESS_DETECT_KIND, None),
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
        address_detect_semantics(),
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
    named_text_port("text", TEXT_VALUE_KIND, direction)
}

fn named_text_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
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
