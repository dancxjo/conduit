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
pub const AUDIO_PRESENT_KIND: &str = "audio/present";
pub const AUDIO_PRESENT_REVISION: &str = "conduit.audio/present@1";
pub const TEXT_VALUE_KIND: &str = "value/text@1";
pub const PCM_AUDIO_VALUE_KIND: &str = "value/audio-pcm@1";
pub const MAXIMUM_TEXT_BYTES: u32 = 256;
pub const MAXIMUM_PCM_BYTES: u32 = 32_768;
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
        outputs: vec![port("audio", PCM_AUDIO_VALUE_KIND, PortDirection::Output)],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_PCM_BYTES,
        },
    }
}

pub fn audio_present_contract() -> SpeechContract {
    SpeechContract {
        kind_id: kind_id(AUDIO_PRESENT_KIND),
        kind_contract_revision: KindContractRevision::from(AUDIO_PRESENT_REVISION),
        inputs: vec![port("audio", PCM_AUDIO_VALUE_KIND, PortDirection::Input)],
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
    for contract in [synthesize_contract(), audio_present_contract()] {
        let is_synthesis = contract.kind_id.as_str() == SPEECH_SYNTHESIZE_KIND;
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
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_contract_contains_no_realization_facts() {
        let encoded = serde_json::to_string(&(synthesize_contract(), audio_present_contract()))
            .expect("contracts serialize");
        for forbidden in ["ALSA", "CPAL", "WebAudio", "WAV", "device", "model", "Base"] {
            assert!(!encoded.contains(forbidden), "found {forbidden}");
        }
    }
}
