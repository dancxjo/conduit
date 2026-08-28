//! Exact portable keyboard text, chord, and typed fan-out contracts.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
    TEXT_PRESENTATION_VALUE_KIND,
};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "form-catalog")]
use conduit_core::KindContractRevision;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_human::{
    CHORD_ENCODED_LEN, CHORD_INFO_ID, CONDUIT_INTL_LAYOUT, CORE_CHORD_MAP, KEY_EVENT_ENCODED_LEN,
    KEY_EVENT_INFO_ID,
};

pub const KEY_EVENT_TEE_KIND: &str = "input/key-tee";
pub const KEY_EVENT_TEE_REVISION: &str = "conduit.input/key-tee@1";

pub const KEYMAP_KIND: &str = "input/keymap";
pub const KEYMAP_REVISION: &str = "conduit.input/keymap@1";

pub const CHORDS_KIND: &str = "input/chords";
pub const CHORDS_REVISION: &str = "conduit.input/chords@1";

pub const INPUT_SEMANTIC_MAXIMUM_VALUES: u16 = 16;

pub const fn key_event_tee_accepts_encoded_len(byte_len: u32) -> bool {
    byte_len == KEY_EVENT_ENCODED_LEN as u32
}

pub fn key_event_tee_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(KEY_EVENT_TEE_KIND),
        plain_name: "Tee key events".to_string(),
        summary: "Deliver each portable key transition atomically to text and chord branches."
            .to_string(),
        inputs: vec![port(
            "key",
            KEY_EVENT_INFO_ID,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![
            port(
                "text-keys",
                KEY_EVENT_INFO_ID,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "chord-keys",
                KEY_EVENT_INFO_ID,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
        ],
        configuration: Vec::new(),
        limits: limits(KEY_EVENT_ENCODED_LEN as u32),
        terminal_behavior: TerminalBehavior::CoupledAtomicFanoutAndMirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "split: input/key-tee".to_string(),
    }
}

pub fn keymap_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(KEYMAP_KIND),
        plain_name: "International QWERTY keymap".to_string(),
        summary: "Interpret portable keys as bounded Unicode text with conduit-intl.".to_string(),
        inputs: vec![port(
            "key",
            KEY_EVENT_INFO_ID,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "text",
            TEXT_PRESENTATION_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Value,
        )],
        configuration: vec![StandardConfigurationField {
            key: "layout".to_string(),
            default_value: ConfigurationValue::Text(CONDUIT_INTL_LAYOUT.to_string()),
            rule: StandardConfigurationRule::TextOneOf {
                values: vec![CONDUIT_INTL_LAYOUT.to_string()],
            },
        }],
        limits: limits(4),
        terminal_behavior: TerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "keymap: input/keymap".to_string(),
    }
}

pub fn chords_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(CHORDS_KIND),
        plain_name: "Keyboard chords".to_string(),
        summary: "Recognize a finite portable command-chord vocabulary without executing it."
            .to_string(),
        inputs: vec![port(
            "key",
            KEY_EVENT_INFO_ID,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "chord",
            CHORD_INFO_ID,
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        configuration: vec![StandardConfigurationField {
            key: "map".to_string(),
            default_value: ConfigurationValue::Text(CORE_CHORD_MAP.to_string()),
            rule: StandardConfigurationRule::TextOneOf {
                values: vec![CORE_CHORD_MAP.to_string()],
            },
        }],
        limits: limits(CHORD_ENCODED_LEN as u32),
        terminal_behavior: TerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "chords: input/chords".to_string(),
    }
}

fn limits(maximum_value_bytes: u32) -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 4,
        max_queue_items: 8,
        max_queue_bytes: 8 * maximum_value_bytes.max(KEY_EVENT_ENCODED_LEN as u32),
    }
}

fn port(
    name: &str,
    info: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_input_semantic_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    for (contract, revision) in [
        (key_event_tee_contract(), KEY_EVENT_TEE_REVISION),
        (keymap_contract(), KEYMAP_REVISION),
        (chords_contract(), CHORDS_REVISION),
    ] {
        let startup_parameters = contract
            .configuration
            .iter()
            .map(|field| StartupParameterSignature {
                name: field.key.clone(),
                value_type: "Text".to_string(),
                default: Some(match &field.default_value {
                    ConfigurationValue::Text(value) => alloc::format!("\"{value}\""),
                    _ => unreachable!("input semantic choices have text defaults"),
                }),
            })
            .collect();
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters,
        })?;
        let configuration = contract
            .configuration
            .iter()
            .map(|field| ConfigurationField {
                key: field.key.clone(),
                default_value: field.default_value.clone(),
                validation: match &field.rule {
                    StandardConfigurationRule::TextOneOf { values } => {
                        ConfigurationRule::TextOneOf {
                            values: values.clone(),
                        }
                    }
                    _ => unreachable!("input semantic configuration uses exact choices"),
                },
            })
            .collect();
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration,
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contracts_are_exact_finite_and_keep_text_and_chords_distinct() {
        let tee = key_event_tee_contract();
        assert_eq!(tee.outputs.len(), 2);
        assert!(tee
            .outputs
            .iter()
            .all(|port| port.value_kind.as_str() == KEY_EVENT_INFO_ID));
        let keymap = keymap_contract();
        assert_eq!(
            keymap.outputs[0].value_kind.as_str(),
            TEXT_PRESENTATION_VALUE_KIND
        );
        assert_eq!(
            keymap.configuration[0].default_value,
            ConfigurationValue::Text(CONDUIT_INTL_LAYOUT.to_string())
        );
        let chords = chords_contract();
        assert_eq!(chords.outputs[0].value_kind.as_str(), CHORD_INFO_ID);
        assert_eq!(
            chords.configuration[0].default_value,
            ConfigurationValue::Text(CORE_CHORD_MAP.to_string())
        );
        assert!(key_event_tee_contract().configuration.is_empty());
        assert_eq!(keymap_contract().configuration.len(), 1);
        assert_eq!(chords_contract().configuration.len(), 1);
    }
}
