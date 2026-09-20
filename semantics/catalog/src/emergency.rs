//! Portable, reusable observation gears for emergency-key recognition.
//!
//! These gears expose bounded detector and sequence semantics to ordinary
//! Forms without granting emergency-control authority. A safety Host may reuse
//! the same implementations below Play, but an ordinary gear output is inert
//! until separately admitted by the out-of-band emergency control plane.

use crate::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
#[cfg(feature = "form-catalog")]
use alloc::format;
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_audio::{AUDIO_PCM_INFO_ID, MAXIMUM_PCM_FRAME_BYTES, PCM_FRAME_HEADER_ENCODED_LEN};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindIdentity, PortDescriptor,
    PortDirection, PortTemporal,
};
#[cfg(feature = "form-catalog")]
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

pub const EMERGENCY_KEYWORD_SPOTTER_KIND: &str = "emergency/keyword-spotter";
pub const EMERGENCY_SEQUENCE_MATCH_KIND: &str = "emergency/sequence-match";
pub const EMERGENCY_KEYWORD_OBSERVATION_INFO_ID: &str = "emergency/keyword-observation@1";
pub const EMERGENCY_TRIGGER_OBSERVATION_INFO_ID: &str = "emergency/trigger-observation@1";
pub const EMERGENCY_KEYWORD_SPOTTER_REVISION: &str = "conduit.std/emergency-keyword-spotter@1";
pub const EMERGENCY_SEQUENCE_MATCH_REVISION: &str = "conduit.std/emergency-sequence-match@1";
pub const EMERGENCY_MAXIMUM_WORD_GAP_MILLIS_KEY: &str = "maximum-word-gap-millis";
pub const EMERGENCY_MAXIMUM_WORD_GAP_MILLIS: u64 = 2_000;
pub const EMERGENCY_KEYWORD_OBSERVATION_MAXIMUM_BYTES: u32 = 32;
pub const EMERGENCY_TRIGGER_OBSERVATION_MAXIMUM_BYTES: u32 = 32;

pub fn emergency_keyword_spotter_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(EMERGENCY_KEYWORD_SPOTTER_KIND),
        plain_name: "Spot a finite emergency keyword".to_string(),
        summary: "Consume bounded PCM and emit only finite-vocabulary keyword observations; exact templates, confidence, storage, and work bounds belong to the selected realization."
            .to_string(),
        inputs: vec![flow_port("audio", AUDIO_PCM_INFO_ID, PortDirection::Input)],
        outputs: vec![value_port(
            "observation",
            EMERGENCY_KEYWORD_OBSERVATION_INFO_ID,
            PortDirection::Output,
        )],
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: PCM_FRAME_HEADER_ENCODED_LEN as u32 + MAXIMUM_PCM_FRAME_BYTES,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "spotter: emergency/keyword-spotter".to_string(),
    }
}

pub fn emergency_sequence_match_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(EMERGENCY_SEQUENCE_MATCH_KIND),
        plain_name: "Match an emergency-key sequence".to_string(),
        summary: "Match one exact finite three-word sequence and emit an inert one-shot trigger observation; this gear neither admits emergency authority nor performs an effect."
            .to_string(),
        inputs: vec![value_port(
            "observation",
            EMERGENCY_KEYWORD_OBSERVATION_INFO_ID,
            PortDirection::Input,
        )],
        outputs: vec![value_port(
            "trigger",
            EMERGENCY_TRIGGER_OBSERVATION_INFO_ID,
            PortDirection::Output,
        )],
        configuration: vec![StandardConfigurationField {
            key: EMERGENCY_MAXIMUM_WORD_GAP_MILLIS_KEY.to_string(),
            default_value: ConfigurationValue::U64(EMERGENCY_MAXIMUM_WORD_GAP_MILLIS),
            rule: StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: EMERGENCY_MAXIMUM_WORD_GAP_MILLIS,
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 3,
            max_queue_bytes: 3 * EMERGENCY_KEYWORD_OBSERVATION_MAXIMUM_BYTES,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "sequence: emergency/sequence-match(maximum-word-gap-millis = 2000)"
            .to_string(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_emergency_observation_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    for (contract, revision) in [
        (
            emergency_keyword_spotter_contract(),
            EMERGENCY_KEYWORD_SPOTTER_REVISION,
        ),
        (
            emergency_sequence_match_contract(),
            EMERGENCY_SEQUENCE_MATCH_REVISION,
        ),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: configuration_type(field).to_string(),
                    default: Some(configuration_source(field)),
                })
                .collect(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindIdentity::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .iter()
                    .map(|field| ConfigurationField {
                        key: field.key.clone(),
                        default_value: field.default_value.clone(),
                        validation: match field.rule {
                            StandardConfigurationRule::U64Range { minimum, maximum } => {
                                ConfigurationRule::U64Range { minimum, maximum }
                            }
                            _ => unreachable!("emergency gears use finite u64 configuration"),
                        },
                    })
                    .collect(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(feature = "form-catalog")]
fn configuration_type(field: &StandardConfigurationField) -> &'static str {
    match &field.default_value {
        ConfigurationValue::U64(_) => "Count",
        _ => unreachable!("emergency gears use finite u64 configuration"),
    }
}

#[cfg(feature = "form-catalog")]
fn configuration_source(field: &StandardConfigurationField) -> String {
    match &field.default_value {
        ConfigurationValue::U64(value) => format!("{value}"),
        _ => unreachable!("emergency gears use finite u64 configuration"),
    }
}

fn value_port(name: &str, info: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn flow_port(name: &str, info: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reusable_gears_are_bounded_observers_without_effect_authority() {
        let spotter = emergency_keyword_spotter_contract();
        let sequence = emergency_sequence_match_contract();
        assert_eq!(spotter.inputs[0].value_kind.as_str(), AUDIO_PCM_INFO_ID);
        assert_eq!(spotter.limits.max_queue_items, 1);
        assert_eq!(sequence.limits.max_queue_items, 3);
        assert!(sequence
            .summary
            .contains("neither admits emergency authority"));
        for contract in [spotter, sequence] {
            let rendered = alloc::format!("{contract:?}").to_ascii_lowercase();
            for forbidden in ["halt", "reset", "wake", "authenticate", "grant authority"] {
                assert!(
                    !rendered.contains(forbidden),
                    "{forbidden} leaked into {rendered}"
                );
            }
        }
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn emergency_observation_gears_expand_as_an_ordinary_form() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        install_emergency_observation_catalogs(&mut startup, &mut profile).unwrap();
        let source = "form emergency_observation (\n > audio: audio/pcm-frames@1\n trigger: emergency/trigger-observation@1 >\n) {\n spotter: emergency/keyword-spotter\n sequence: emergency/sequence-match(maximum-word-gap-millis = 2000)\n audio > spotter.audio\n spotter.observation > sequence.observation\n sequence.trigger > trigger\n}\n";
        let syntax = conduit_form::parse_syntax_document(source);
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        let expanded = conduit_form::expand_canonical_form_for_authoring(
            &checked,
            "emergency_observation",
            &profile,
        )
        .unwrap();
        assert_eq!(expanded.expanded.gears.len(), 2);
    }
}
