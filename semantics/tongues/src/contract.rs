use conduit_audio::AUDIO_PCM_INFO_ID;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};
use serde::{Deserialize, Serialize};

pub const SPEECH_SYNTHESIZE_KIND: &str = "speech/synthesize";
pub const SPEECH_SYNTHESIZE_REVISION: &str = "conduit.speech/synthesize@1";
pub const SPEECH_SYNTHESIZE_STREAM_KIND: &str = "speech/synthesize-stream";
pub const SPEECH_SYNTHESIZE_STREAM_REVISION: &str = "conduit.speech/synthesize-stream@1";
pub const AUDIO_PLAY_KIND: &str = "audio/play";
pub const AUDIO_PLAY_REVISION: &str = "conduit.std/audio-play@1";
pub const TEXT_VALUE_KIND: &str = "value/text@1";
pub const MAXIMUM_TEXT_BYTES: u32 = 256;
/// At most about three seconds of signed 16-bit mono speech at 22.05 kHz.
pub const MAXIMUM_PCM_BYTES: u32 = 131_072;
pub const MAXIMUM_AUDIO_FRAMES: u32 = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

pub fn synthesize_contract() -> SpeechContract {
    SpeechContract {
        kind_id: kind_id(SPEECH_SYNTHESIZE_KIND),
        kind_contract_revision: KindContractRevision::from(SPEECH_SYNTHESIZE_REVISION),
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
        kind_id: kind_id(SPEECH_SYNTHESIZE_STREAM_KIND),
        kind_contract_revision: KindContractRevision::from(SPEECH_SYNTHESIZE_STREAM_REVISION),
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

pub fn audio_play_contract() -> SpeechContract {
    SpeechContract {
        kind_id: kind_id(AUDIO_PLAY_KIND),
        kind_contract_revision: KindContractRevision::from(AUDIO_PLAY_REVISION),
        inputs: vec![port("audio", AUDIO_PCM_INFO_ID, PortDirection::Input)],
        outputs: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_PCM_BYTES,
        },
    }
}

pub fn install_speech_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    install_speech_synthesis_catalog(startup, profile)?;
    install_speech_commit_catalog(startup, profile)?;
    install_contract(startup, profile, audio_play_contract(), false)?;
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
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

pub fn install_speech_synthesis_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    install_contract(startup, profile, synthesize_contract(), true)?;
    install_contract(startup, profile, streaming_synthesize_contract(), true)
}

fn install_contract(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    contract: SpeechContract,
    is_synthesis: bool,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: contract.kind_id.as_str().into(),
        startup_parameters: if is_synthesis {
            vec![StartupParameterSignature {
                name: "maximum-output-bytes".into(),
                value_type: "Count".into(),
                default: Some(MAXIMUM_PCM_BYTES.to_string()),
            }]
        } else {
            vec![]
        },
    })?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: if is_synthesis {
                vec![ConfigurationField {
                    key: "maximum-output-bytes".into(),
                    default_value: conduit_core::ConfigurationValue::U64(u64::from(
                        MAXIMUM_PCM_BYTES,
                    )),
                    validation: ConfigurationRule::U64Range {
                        minimum: 1,
                        maximum: u64::from(MAXIMUM_PCM_BYTES),
                    },
                }]
            } else {
                vec![]
            },
        })
        .map_err(|error| error.to_string())
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn flow_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_contract_contains_no_realization_facts() {
        let encoded = serde_json::to_string(&(synthesize_contract(), audio_play_contract()))
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
