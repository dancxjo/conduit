//! Exact portable keyboard text, chord, and typed fan-out contracts.

use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
    TEXT_FACE_VALUE_KIND,
};
#[cfg(feature = "plot-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, Kind, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_human::{
    CHORD_ENCODED_LEN, CHORD_INFO_ID, CORE_CHORD_MAP, KEY_EVENT_ENCODED_LEN, KEY_EVENT_INFO_ID,
};

pub const KEY_EVENT_TEE_KIND: &str = "input/key-tee";
pub const KEY_EVENT_TEE_REVISION: &str = "conduit.input/key-tee@2";

pub const KEYMAP_KIND: &str = "input/keymap";
pub const KEYMAP_REVISION: &str = "conduit.input/keymap@3";

pub const CHORDS_KIND: &str = "input/chords";
pub const CHORDS_REVISION: &str = "conduit.input/chords@2";

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
            PortTemporal::Flow { closes: false },
        )],
        outputs: vec![
            port(
                "text-keys",
                KEY_EVENT_INFO_ID,
                PortDirection::Output,
                PortTemporal::Flow { closes: false },
            ),
            port(
                "chord-keys",
                KEY_EVENT_INFO_ID,
                PortDirection::Output,
                PortTemporal::Flow { closes: false },
            ),
        ],
        configuration: Default::default(),
        limits: limits(KEY_EVENT_ENCODED_LEN as u32),
        terminal_behavior: KindTerminalBehavior::CoupledAtomicFanoutAndMirrorsInputTerminal,
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
        summary: "Interpret portable keys through authored layered US International mappings."
            .to_string(),
        inputs: vec![
            port(
                "key",
                KEY_EVENT_INFO_ID,
                PortDirection::Input,
                PortTemporal::Flow { closes: false },
            ),
            port(
                "selection",
                TEXT_FACE_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Current,
            ),
        ],
        outputs: vec![port(
            "text",
            TEXT_FACE_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Flow { closes: false },
        )],
        configuration: vec![
            KindConfigurationField {
                key: "layout".to_string(),
                default_value: ConfigurationValue::Structured(
                    conduit_human::default_keymap_configuration(),
                ),
                rule: KindConfigurationRule::Structured {
                    profile: conduit_human::keymap_layout_type()
                        .profile()
                        .expect("finite keymap layout")
                        .value_kind()
                        .clone(),
                },
            },
            KindConfigurationField {
                key: "choices".to_string(),
                default_value: ConfigurationValue::Structured(
                    conduit_human::default_choices_configuration(),
                ),
                rule: KindConfigurationRule::Structured {
                    profile: conduit_human::choices_type()
                        .profile()
                        .expect("finite choices")
                        .value_kind()
                        .clone(),
                },
            },
        ],
        limits: limits(conduit_human::KEYMAP_MAXIMUM_TEXT_BYTES as u32),
        terminal_behavior: KindTerminalBehavior::MirrorsInputTerminal,
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
            PortTemporal::Flow { closes: false },
        )],
        outputs: vec![port(
            "chord",
            CHORD_INFO_ID,
            PortDirection::Output,
            PortTemporal::Flow { closes: false },
        )],
        configuration: vec![KindConfigurationField {
            key: "map".to_string(),
            default_value: ConfigurationValue::Text(CORE_CHORD_MAP.to_string()),
            rule: KindConfigurationRule::TextOneOf {
                values: vec![CORE_CHORD_MAP.to_string()],
            },
        }],
        limits: limits(CHORD_ENCODED_LEN as u32),
        terminal_behavior: KindTerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "chords: input/chords".to_string(),
    }
}

pub fn key_event_tee_semantic_contract() -> Kind {
    semantic_contract(key_event_tee_contract(), KEY_EVENT_TEE_REVISION)
}

pub fn keymap_semantic_contract() -> Kind {
    semantic_contract(keymap_contract(), KEYMAP_REVISION)
}

pub fn chords_semantic_contract() -> Kind {
    semantic_contract(chords_contract(), CHORDS_REVISION)
}

fn semantic_contract(contract: StandardKindContract, revision: &str) -> Kind {
    Kind {
        startup_parameters: super::startup_front(&contract.configuration),
        shorthand: None,
        kind_id: contract.kind_id,
        kind_contract_revision: revision.into(),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract.configuration,
        semantic_laws: alloc::vec![conduit_core::KindSemanticLaw::Terminal(
            contract.terminal_behavior
        )],
        limits: contract.limits,
    }
}

fn limits(maximum_value_bytes: u32) -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 4,
        max_queue_items: 8,
        max_queue_bytes: (8 * maximum_value_bytes.max(KEY_EVENT_ENCODED_LEN as u32)).max(64),
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
        abnormal_kind: None,
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_input_semantic_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    use conduit_plot::{KindSignature, StartupParameterSignature};
    startup.insert_structured_type("KeymapLayout", conduit_human::keymap_layout_type())?;
    startup.insert_structured_type("KeymapChoices", conduit_human::choices_type())?;
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
                value_type: if field.key == "choices" {
                    "KeymapChoices"
                } else if contract.kind_id.as_str() == KEYMAP_KIND {
                    "KeymapLayout"
                } else {
                    "Text"
                }
                .to_string(),
                default: Some(match &field.default_value {
                    ConfigurationValue::Text(value) => alloc::format!("\"{value}\""),
                    ConfigurationValue::Structured(_) => "<checked-structured-default>".to_string(),
                    _ => unreachable!("input semantic defaults are authored values"),
                }),
            })
            .collect();
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters,
        })?;
        for field in &contract.configuration {
            if let ConfigurationValue::Structured(value) = &field.default_value {
                let checked = conduit_core::StructuredInfoValue::from_canonical_bytes(
                    value.canonical_value(),
                )
                .map_err(|error| alloc::format!("checked keymap default: {error:?}"))?;
                startup.insert_structured_default(
                    contract.kind_id.as_str(),
                    &field.key,
                    &checked,
                )?;
            }
        }
        profile
            .insert_kind(semantic_contract(contract, revision))
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
        assert_eq!(keymap.outputs[0].value_kind.as_str(), TEXT_FACE_VALUE_KIND);
        assert_eq!(
            keymap.configuration[0].default_value,
            ConfigurationValue::Structured(conduit_human::default_keymap_configuration())
        );
        let chords = chords_contract();
        assert_eq!(chords.outputs[0].value_kind.as_str(), CHORD_INFO_ID);
        assert_eq!(
            chords.configuration[0].default_value,
            ConfigurationValue::Text(CORE_CHORD_MAP.to_string())
        );
        assert!(key_event_tee_contract().configuration.is_empty());
        assert_eq!(keymap_contract().configuration.len(), 2);
        assert_eq!(chords_contract().configuration.len(), 1);
        for (contract, revision, startup_count) in [
            (key_event_tee_semantic_contract(), KEY_EVENT_TEE_REVISION, 0),
            (keymap_semantic_contract(), KEYMAP_REVISION, 2),
            (chords_semantic_contract(), CHORDS_REVISION, 1),
        ] {
            assert_eq!(contract.kind_contract_revision.as_str(), revision);
            assert_eq!(contract.startup_parameters.len(), startup_count);
            assert!(contract
                .startup_parameters
                .iter()
                .all(|parameter| parameter.has_default));
        }
    }
}
