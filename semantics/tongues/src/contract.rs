use conduit_audio::AUDIO_PCM_INFO_ID;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, FrontStartupParameter, Kind, KindId, KindIdentity,
    PortDescriptor, PortDirection, PortTemporal,
};
use conduit_plot::{
    KindConfigurationField, KindConfigurationRule, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use serde::{Deserialize, Serialize};

pub const SPEECH_SYNTHESIZE_KIND: &str = "speech/synthesize";
pub const SPEECH_SYNTHESIZE_REVISION: &str = "conduit.speech/synthesize@2";
pub const SPEECH_SYNTHESIZE_STREAM_KIND: &str = "speech/synthesize-stream";
pub const SPEECH_SYNTHESIZE_STREAM_REVISION: &str = "conduit.speech/synthesize-stream@3";
pub const AUDIO_PLAY_KIND: &str = "audio/play";
pub const AUDIO_PLAY_REVISION: &str = conduit_semantic_catalog::AUDIO_PLAY_REVISION;
pub const TEXT_VALUE_KIND: &str = "value/text";
pub const MAXIMUM_TEXT_BYTES: u32 = 256;
/// At most about three seconds of signed 16-bit mono speech at 22.05 kHz.
pub const MAXIMUM_PCM_BYTES: u32 = 131_072;
pub const MAXIMUM_AUDIO_FRAMES: u32 = 16_384;
/// Stream @2 admits total work separately from one in-flight PCM block.
pub const MAXIMUM_STREAM_PCM_BYTES: u32 = 1_323_000;
pub const MAXIMUM_STREAM_AUDIO_MILLIS: u32 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechContract {
    pub startup_parameters: Vec<FrontStartupParameter>,
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

impl SpeechContract {
    pub fn into_semantic_capability_contract(self) -> Kind {
        Kind {
            startup_parameters: self.startup_parameters,
            shorthand: None,
            kind_id: self.kind_id,
            kind_contract_revision: self.kind_contract_revision,
            inputs: self.inputs,
            outputs: self.outputs,
            configuration: Default::default(),
            semantic_laws: Default::default(),
            limits: self.limits,
        }
    }
}

fn synthesis_startup_parameters() -> Vec<FrontStartupParameter> {
    vec![
        FrontStartupParameter {
            name: "maximum-output-bytes".into(),
            value_type: kind_id("value/count"),
            has_default: true,
        },
        conduit_language::language_request_parameter(),
    ]
}

pub fn synthesize_contract() -> SpeechContract {
    SpeechContract {
        startup_parameters: synthesis_startup_parameters(),
        kind_id: kind_id(SPEECH_SYNTHESIZE_KIND),
        kind_contract_revision: KindIdentity::from(SPEECH_SYNTHESIZE_REVISION),
        inputs: vec![port("text", TEXT_VALUE_KIND, PortDirection::Input)],
        outputs: vec![port("audio", AUDIO_PCM_INFO_ID, PortDirection::Output)],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_PCM_BYTES,
        },
    }
}

pub fn streaming_synthesize_contract() -> SpeechContract {
    SpeechContract {
        startup_parameters: synthesis_startup_parameters(),
        kind_id: kind_id(SPEECH_SYNTHESIZE_STREAM_KIND),
        kind_contract_revision: KindIdentity::from(SPEECH_SYNTHESIZE_STREAM_REVISION),
        inputs: vec![flow_port(
            "text",
            crate::SPEAKABLE_TEXT_VALUE_KIND,
            PortDirection::Input,
        )],
        outputs: vec![flow_port("audio", AUDIO_PCM_INFO_ID, PortDirection::Output)],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: crate::MAXIMUM_COMMITTED_SEGMENTS as u16,
            max_queue_bytes: MAXIMUM_PCM_BYTES,
        },
    }
}

pub fn synthesize_semantic_contract() -> Kind {
    synthesis_semantic_contract(synthesize_contract())
}

/// Stream @2 counts PCM payload bytes and decoded audio duration across all
/// committed segments, without resetting either total at a segment boundary.
/// Limits are ceilings: excess output is refused, never silently truncated.
/// The selected back separately admits fixed per-block storage and finite work.
pub fn streaming_synthesize_semantic_contract() -> Kind {
    let mut kind = synthesis_semantic_contract(streaming_synthesize_contract());
    kind.configuration[0].rule = KindConfigurationRule::U64Range {
        minimum: 1,
        maximum: u64::from(MAXIMUM_STREAM_PCM_BYTES),
    };
    for (key, default, maximum) in [
        (
            "maximum-audio-millis",
            3_000,
            u64::from(MAXIMUM_STREAM_AUDIO_MILLIS),
        ),
        (
            "maximum-segments",
            crate::MAXIMUM_COMMITTED_SEGMENTS as u64,
            crate::MAXIMUM_COMMITTED_SEGMENTS as u64,
        ),
    ] {
        kind.startup_parameters.push(FrontStartupParameter {
            name: key.into(),
            value_type: kind_id("value/count"),
            has_default: true,
        });
        kind.configuration.push(KindConfigurationField {
            key: key.into(),
            default_value: conduit_core::ConfigurationValue::U64(default),
            rule: KindConfigurationRule::U64Range {
                minimum: 1,
                maximum,
            },
        });
    }
    kind
}

fn synthesis_semantic_contract(contract: SpeechContract) -> Kind {
    let mut kind = contract.into_semantic_capability_contract();
    kind.configuration = vec![KindConfigurationField {
        key: "maximum-output-bytes".into(),
        default_value: conduit_core::ConfigurationValue::U64(u64::from(MAXIMUM_PCM_BYTES)),
        rule: KindConfigurationRule::U64Range {
            minimum: 1,
            maximum: u64::from(MAXIMUM_PCM_BYTES),
        },
    }];
    kind.configuration
        .push(conduit_language::language_request_field());
    kind.semantic_laws = conduit_language::language_requirement_laws();
    kind
}

pub fn audio_play_contract() -> SpeechContract {
    let kind =
        conduit_semantic_catalog::audio_play_contract().into_semantic_contract(AUDIO_PLAY_REVISION);
    SpeechContract {
        startup_parameters: kind.startup_parameters,
        kind_id: kind.kind_id,
        kind_contract_revision: kind.kind_contract_revision,
        inputs: kind.inputs,
        outputs: kind.outputs,
        limits: kind.limits,
    }
}

pub fn install_speech_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    install_speech_synthesis_catalog(startup, profile)?;
    install_speech_commit_catalog(startup, profile)?;
    conduit_semantic_catalog::install_audio_play_catalog(startup, profile)?;
    Ok(())
}

pub fn install_speech_commit_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    let contract = crate::speech_commit_contract();
    startup.insert(KindSignature {
        kind: contract.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(crate::speech_commit_semantic_contract())
        .map_err(|error| error.to_string())
}

pub fn install_speech_synthesis_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    conduit_language::install_language_request_type(startup)?;
    install_contract(startup, profile, synthesize_contract(), true)?;
    install_contract(startup, profile, streaming_synthesize_contract(), true)
}

fn install_contract(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    contract: SpeechContract,
    is_synthesis: bool,
) -> Result<(), String> {
    let kind = if is_synthesis {
        if contract.kind_id.as_str() == SPEECH_SYNTHESIZE_STREAM_KIND {
            streaming_synthesize_semantic_contract()
        } else {
            synthesis_semantic_contract(contract)
        }
    } else {
        contract.into_semantic_capability_contract()
    };
    startup.insert(KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: kind
            .configuration
            .iter()
            .map(|field| {
                if field.key == "language-request" {
                    return conduit_language::language_request_signature();
                }
                StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: "Count".into(),
                    default: match field.default_value {
                        conduit_core::ConfigurationValue::U64(value) => Some(value.to_string()),
                        _ => None,
                    },
                }
            })
            .collect(),
    })?;
    profile.insert_kind(kind).map_err(|error| error.to_string())
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

fn flow_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_contract_contains_no_realization_facts() {
        let synthesis = synthesize_contract();
        assert_eq!(synthesis.startup_parameters.len(), 1);
        assert_eq!(synthesis.startup_parameters[0].name, "maximum-output-bytes");
        assert_eq!(
            synthesis
                .clone()
                .into_semantic_capability_contract()
                .startup_parameters,
            synthesis.startup_parameters
        );
        let encoded = serde_json::to_string(&(synthesis, audio_play_contract()))
            .expect("contracts serialize");
        for forbidden in ["ALSA", "CPAL", "WebAudio", "WAV", "device", "model", "Base"] {
            assert!(!encoded.contains(forbidden), "found {forbidden}");
        }
    }

    #[test]
    fn streaming_synthesis_consumes_segments_and_emits_ordered_audio_flow() {
        let contract = streaming_synthesize_contract();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            crate::SPEAKABLE_TEXT_VALUE_KIND
        );
        assert_eq!(
            contract.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(contract.outputs[0].value_kind.as_str(), AUDIO_PCM_INFO_ID);
        assert_eq!(
            contract.outputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
    }
}
