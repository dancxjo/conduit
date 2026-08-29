//! Typed finite verb contracts used by recursive Morse Forms.

use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal,
};

use crate::{
    MorseKindContract, DEFAULT_MORSE_UNIT_MILLIS, MAXIMUM_MORSE_CHARACTERS_BYTES,
    MAXIMUM_MORSE_GAPPED_GROUPS_BYTES, MAXIMUM_MORSE_PATTERN_BYTES, MAXIMUM_MORSE_SYMBOLS_BYTES,
    MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES, MAXIMUM_MORSE_UNIT_MILLIS, MINIMUM_MORSE_UNIT_MILLIS,
    MORSE_CHARACTERS_VALUE_KIND, MORSE_GAPPED_GROUPS_VALUE_KIND, MORSE_PATTERN_VALUE_KIND,
    MORSE_SYMBOLS_VALUE_KIND, MORSE_SYMBOL_GROUPS_VALUE_KIND, MORSE_UNIT_MILLIS_KEY,
    TEXT_VALUE_KIND,
};

pub const TEXT_CHARACTERS_KIND: &str = "text/characters";
pub const TEXT_MORSE_SYMBOLS_KIND: &str = "text/morse-symbols";
pub const MORSE_LOOKUP_KIND: &str = "morse/lookup";
pub const MORSE_INTERSPERSE_KIND: &str = "morse/intersperse";
pub const MORSE_FLATTEN_KIND: &str = "morse/flatten";
pub const MORSE_SYMBOLS_TO_PATTERN_KIND: &str = "morse/symbols-to-pattern";
pub const MORSE_PATTERN_TO_SYMBOLS_KIND: &str = "morse/pattern-to-symbols";
pub const MORSE_SYMBOLS_TO_TEXT_KIND: &str = "morse/symbols-to-text";

pub const MORSE_COMPOSITION_CONTRACT_REVISION: &str = "conduit.morse/composition@1";

pub fn text_characters_semantics() -> MorseKindContract {
    contract_with_ports(
        TEXT_CHARACTERS_KIND,
        "in",
        TEXT_VALUE_KIND,
        "out",
        MORSE_CHARACTERS_VALUE_KIND,
        None,
    )
}

pub fn text_morse_symbols_semantics() -> MorseKindContract {
    contract_with_ports(
        TEXT_MORSE_SYMBOLS_KIND,
        "text",
        TEXT_VALUE_KIND,
        "symbols",
        MORSE_SYMBOLS_VALUE_KIND,
        None,
    )
}

pub fn morse_lookup_semantics() -> MorseKindContract {
    contract(
        MORSE_LOOKUP_KIND,
        MORSE_CHARACTERS_VALUE_KIND,
        MORSE_SYMBOL_GROUPS_VALUE_KIND,
        None,
    )
}

pub fn morse_intersperse_semantics() -> MorseKindContract {
    contract(
        MORSE_INTERSPERSE_KIND,
        MORSE_SYMBOL_GROUPS_VALUE_KIND,
        MORSE_GAPPED_GROUPS_VALUE_KIND,
        None,
    )
}

pub fn morse_flatten_semantics() -> MorseKindContract {
    contract(
        MORSE_FLATTEN_KIND,
        MORSE_GAPPED_GROUPS_VALUE_KIND,
        MORSE_SYMBOLS_VALUE_KIND,
        None,
    )
}

pub fn morse_symbols_to_pattern_semantics() -> MorseKindContract {
    contract(
        MORSE_SYMBOLS_TO_PATTERN_KIND,
        MORSE_SYMBOLS_VALUE_KIND,
        MORSE_PATTERN_VALUE_KIND,
        Some((
            MORSE_UNIT_MILLIS_KEY,
            ConfigurationValue::U64(u64::from(DEFAULT_MORSE_UNIT_MILLIS)),
        )),
    )
}

pub fn morse_pattern_to_symbols_semantics() -> MorseKindContract {
    contract(
        MORSE_PATTERN_TO_SYMBOLS_KIND,
        MORSE_PATTERN_VALUE_KIND,
        MORSE_SYMBOLS_VALUE_KIND,
        None,
    )
}

pub fn morse_symbols_to_text_semantics() -> MorseKindContract {
    contract(
        MORSE_SYMBOLS_TO_TEXT_KIND,
        MORSE_SYMBOLS_VALUE_KIND,
        TEXT_VALUE_KIND,
        None,
    )
}

fn contract(
    kind: &str,
    input_kind: &str,
    output_kind: &str,
    configuration: Option<(&'static str, ConfigurationValue)>,
) -> MorseKindContract {
    contract_with_ports(kind, "in", input_kind, "out", output_kind, configuration)
}

fn contract_with_ports(
    kind: &str,
    input_port: &str,
    input_kind: &str,
    output_port: &str,
    output_kind: &str,
    configuration: Option<(&'static str, ConfigurationValue)>,
) -> MorseKindContract {
    MorseKindContract {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(MORSE_COMPOSITION_CONTRACT_REVISION),
        inputs: vec![port(input_port, input_kind, PortDirection::Input)],
        outputs: vec![port(output_port, output_kind, PortDirection::Output)],
        configuration: configuration.into_iter().collect(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: maximum_bytes(input_kind).max(maximum_bytes(output_kind)) as u32,
        },
    }
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn maximum_bytes(value_kind: &str) -> usize {
    match value_kind {
        TEXT_VALUE_KIND => crate::MAX_TEXT_BYTES as usize,
        MORSE_CHARACTERS_VALUE_KIND => MAXIMUM_MORSE_CHARACTERS_BYTES,
        MORSE_SYMBOL_GROUPS_VALUE_KIND => MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES,
        MORSE_GAPPED_GROUPS_VALUE_KIND => MAXIMUM_MORSE_GAPPED_GROUPS_BYTES,
        MORSE_SYMBOLS_VALUE_KIND => MAXIMUM_MORSE_SYMBOLS_BYTES,
        MORSE_PATTERN_VALUE_KIND => MAXIMUM_MORSE_PATTERN_BYTES,
        _ => 0,
    }
}

#[cfg(feature = "form-catalog")]
pub(crate) fn install_morse_composition_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };

    for contract in [
        text_characters_semantics(),
        text_morse_symbols_semantics(),
        morse_lookup_semantics(),
        morse_intersperse_semantics(),
        morse_flatten_semantics(),
        morse_symbols_to_pattern_semantics(),
        morse_pattern_to_symbols_semantics(),
        morse_symbols_to_text_semantics(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|(name, value)| StartupParameterSignature {
                    name: (*name).to_string(),
                    value_type: "Count".to_string(),
                    default: match value {
                        ConfigurationValue::U64(value) => Some(value.to_string()),
                        _ => None,
                    },
                })
                .collect(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .into_iter()
                    .map(|(key, value)| ConfigurationField {
                        key: key.to_string(),
                        default_value: value,
                        validation: ConfigurationRule::U64Range {
                            minimum: u64::from(MINIMUM_MORSE_UNIT_MILLIS),
                            maximum: u64::from(MAXIMUM_MORSE_UNIT_MILLIS),
                        },
                    })
                    .collect(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_verbs_are_pairwise_typed_and_finite() {
        let contracts = [
            text_characters_semantics(),
            text_morse_symbols_semantics(),
            morse_lookup_semantics(),
            morse_intersperse_semantics(),
            morse_flatten_semantics(),
            morse_symbols_to_pattern_semantics(),
            morse_pattern_to_symbols_semantics(),
            morse_symbols_to_text_semantics(),
        ];
        assert!(contracts.iter().all(|contract| {
            contract.inputs.len() == 1
                && contract.outputs.len() == 1
                && contract.limits.max_queue_items == 1
                && contract.limits.max_queue_bytes > 0
        }));
        assert_eq!(
            text_morse_symbols_semantics().outputs[0].value_kind,
            morse_symbols_to_pattern_semantics().inputs[0].value_kind
        );
    }
}
