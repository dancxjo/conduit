//! Exact sound and music implementation offers owned by the hosted std Host.

use conduit_audio::{
    AUDIO_PCM_INFO_ID, CONTROL_EVENT_ENCODED_LEN, MUSIC_CONTROL_INFO_ID, MUSIC_NOTE_INFO_ID,
    NOTE_EVENT_ENCODED_LEN,
};
use conduit_core::{
    kind_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const MUSIC_PLAY_MIDI_PROFILE: &str = "std/midi1-channel-12tet-a440-output@1";
pub const MUSIC_PLAY_MIDI_IMPLEMENTATION: &str = "std/kernel-music-play-midi1@1";
pub const MUSIC_PLAY_MIDI_ARTIFACT: &str = "conduit-std-host/music-play-midi1@1";
pub const MUSIC_PLAY_MIDI_NOTE_OPERATION: &str = "conduit.host/midi1-output-note@1";
pub const MUSIC_PLAY_MIDI_CONTROL_OPERATION: &str = "conduit.host/midi1-output-control@1";
pub const MIDI_OUTPUT_RESOURCE_CLASS: &str = "conduit.resource/midi-output@1";
pub const MIDI_OUTPUT_AUTHORITY_CONTRACT: &str = "conduit.authority/midi-output@1";

pub const MUSIC_SYNTH_REFERENCE_PROFILE: &str = "conduit.reference/music-synth-fixed-q16@1";
pub const MUSIC_SYNTH_REFERENCE_IMPLEMENTATION: &str = "std/kernel-music-synth-fixed-q16@1";
pub const MUSIC_SYNTH_REFERENCE_ARTIFACT: &str = "conduit-std-host/music-synth-fixed-q16@1";
pub const MUSIC_SYNTH_HOST_OPERATION: &str = "conduit.host/music-synth-render-fixed-q16@1";

pub const AUDIO_PLAY_ALSA_HW_PROFILE: &str = "std/alsa-hw-s16le-48000-stereo-p256-b1024@1";
pub const AUDIO_PLAY_ALSA_HW_IMPLEMENTATION: &str = "std/kernel-audio-play-alsa-hw@1";
pub const AUDIO_PLAY_ALSA_HW_ARTIFACT: &str = "conduit-std-host/alsa-aplay-hw@1";
pub const AUDIO_PLAY_ALSA_HW_OPERATION: &str = "conduit.host/audio-play-alsa-hw@1";
pub const AUDIO_PLAYBACK_RESOURCE_CLASS: &str = "conduit.resource/audio-playback-alsa-hw@1";
pub const AUDIO_PLAYBACK_AUTHORITY_CONTRACT: &str = "conduit.authority/audio-playback@1";

pub const MUSIC_INPUT_MIDI_PROFILE: &str = "std/midi1-raw-input-monotonic-us@1";
pub const MUSIC_INPUT_MIDI_IMPLEMENTATION: &str = "std/kernel-music-input-midi1@1";
pub const MUSIC_INPUT_MIDI_ARTIFACT: &str = "conduit-std-host/music-input-midi1@1";
pub const MUSIC_INPUT_MIDI_OPERATION: &str = "conduit.host/midi1-input-next-observation@1";
pub const MIDI_INPUT_RESOURCE_CLASS: &str = "conduit.resource/midi-input@1";
pub const MIDI_INPUT_AUTHORITY_CONTRACT: &str = "conduit.authority/midi-input@1";

pub const INSTRUMENT_MAP_STD_PROFILE: &str = "std/instrument-map-kernel@1";
pub const INSTRUMENT_MAP_STD_IMPLEMENTATION: &str = "std/kernel-music-instrument-map@1";
pub const INSTRUMENT_MAP_STD_ARTIFACT: &str = "conduit-std-host/music-instrument-map@1";
pub const RHYTHM_COMPARE_STD_PROFILE: &str = "std/music-rhythm-compare-kernel-hosted@1";
pub const RHYTHM_COMPARE_STD_IMPLEMENTATION: &str = "std/kernel-music-rhythm-compare@1";
pub const RHYTHM_COMPARE_STD_ARTIFACT: &str = "conduit-std-host/music-rhythm-compare@1";
pub const RHYTHM_PERFORMANCE_HOST_OPERATION: &str = "conduit.host/music-rhythm-performance@1";
pub const RHYTHM_REFERENCE_HOST_OPERATION: &str = "conduit.host/music-rhythm-reference@1";
pub const RHYTHM_DRAIN_HOST_OPERATION: &str = "conduit.host/music-rhythm-drain@1";

pub fn music_play_midi_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::music_play_contract();
    let operations = [
        (
            MUSIC_PLAY_MIDI_CONTROL_OPERATION,
            MUSIC_CONTROL_INFO_ID,
            CONTROL_EVENT_ENCODED_LEN,
        ),
        (
            MUSIC_PLAY_MIDI_NOTE_OPERATION,
            MUSIC_NOTE_INFO_ID,
            NOTE_EVENT_ENCODED_LEN,
        ),
    ];
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("music-play-midi1"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::MUSIC_PLAY_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: implementation(
            MUSIC_PLAY_MIDI_PROFILE,
            MUSIC_PLAY_MIDI_IMPLEMENTATION,
            MUSIC_PLAY_MIDI_ARTIFACT,
        ),
        host_operations: operations
            .iter()
            .map(|(id, kind, bytes)| HostOperationRequirement {
                contract_id: HostOperationContractId::from(*id),
                target_kind: Some(kind_id(kind)),
                maximum_in_flight: 1,
                maximum_input_bytes: *bytes as u32,
                maximum_output_bytes: 0,
            })
            .collect(),
        resource_requirements: vec![resource_requirement(MIDI_OUTPUT_RESOURCE_CLASS, 1)],
        authority_requirements: operations
            .iter()
            .map(|(id, kind, _)| AuthorityRequirement {
                contract_id: AuthorityContractId::from(MIDI_OUTPUT_AUTHORITY_CONTRACT),
                host_operation_contract_id: HostOperationContractId::from(*id),
                subject_kind: kind_id(kind),
            })
            .collect(),
        limits: contract.limits,
    }
}

pub fn music_synth_reference_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::music_synth_contract();
    CapabilityOffer {
        startup_parameters: conduit_semantic_catalog::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from("music-synth-fixed-q16"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::MUSIC_SYNTH_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: implementation(
            MUSIC_SYNTH_REFERENCE_PROFILE,
            MUSIC_SYNTH_REFERENCE_IMPLEMENTATION,
            MUSIC_SYNTH_REFERENCE_ARTIFACT,
        ),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(MUSIC_SYNTH_HOST_OPERATION),
            target_kind: Some(kind_id(AUDIO_PCM_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: NOTE_EVENT_ENCODED_LEN.max(CONTROL_EVENT_ENCODED_LEN) as u32,
            maximum_output_bytes: conduit_semantic_catalog::MUSIC_SYNTH_PCM_BLOCK_BYTES,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

pub fn audio_play_alsa_hw_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::audio_play_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("audio-play-alsa-hw"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::AUDIO_PLAY_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: implementation(
            AUDIO_PLAY_ALSA_HW_PROFILE,
            AUDIO_PLAY_ALSA_HW_IMPLEMENTATION,
            AUDIO_PLAY_ALSA_HW_ARTIFACT,
        ),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(AUDIO_PLAY_ALSA_HW_OPERATION),
            target_kind: Some(kind_id(AUDIO_PCM_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![resource_requirement(AUDIO_PLAYBACK_RESOURCE_CLASS, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(AUDIO_PLAYBACK_AUTHORITY_CONTRACT),
            host_operation_contract_id: HostOperationContractId::from(AUDIO_PLAY_ALSA_HW_OPERATION),
            subject_kind: kind_id(AUDIO_PCM_INFO_ID),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: conduit_semantic_catalog::MAXIMUM_AUDIO_QUEUE_ITEMS,
            max_queue_bytes: conduit_semantic_catalog::MAXIMUM_AUDIO_QUEUE_BYTES,
        },
    }
}

pub fn music_input_midi_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::music_input_contract();
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(MUSIC_INPUT_MIDI_OPERATION),
        target_kind: Some(kind_id(conduit_midi::MIDI_INPUT_OBSERVATION_INFO_ID)),
        maximum_in_flight: 1,
        maximum_input_bytes: 0,
        maximum_output_bytes: conduit_midi::MIDI_INPUT_OBSERVATION_ENCODED_LEN as u32,
    };
    CapabilityOffer {
        startup_parameters: conduit_semantic_catalog::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from("music-input-midi1"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::MUSIC_INPUT_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: implementation(
            MUSIC_INPUT_MIDI_PROFILE,
            MUSIC_INPUT_MIDI_IMPLEMENTATION,
            MUSIC_INPUT_MIDI_ARTIFACT,
        ),
        host_operations: vec![operation.clone()],
        resource_requirements: vec![resource_requirement(MIDI_INPUT_RESOURCE_CLASS, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(MIDI_INPUT_AUTHORITY_CONTRACT),
            host_operation_contract_id: operation.contract_id,
            subject_kind: kind_id(conduit_midi::MIDI_INPUT_OBSERVATION_INFO_ID),
        }],
        limits: contract.limits,
    }
}

pub fn rhythm_compare_std_offer() -> CapabilityOffer {
    let definition = conduit_semantic_catalog::rhythm_compare_definition();
    let target_kind = definition.kind_id.clone();
    CapabilityOffer {
        startup_parameters: vec![
            startup("target-offset-micros", "Scalar", true),
            startup("tolerance-micros", "Count", true),
        ],
        shorthand: None,
        capability_id: CapabilityId::from("music-rhythm-compare"),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        implementation: implementation(
            RHYTHM_COMPARE_STD_PROFILE,
            RHYTHM_COMPARE_STD_IMPLEMENTATION,
            RHYTHM_COMPARE_STD_ARTIFACT,
        ),
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: [
            RHYTHM_DRAIN_HOST_OPERATION,
            RHYTHM_PERFORMANCE_HOST_OPERATION,
            RHYTHM_REFERENCE_HOST_OPERATION,
        ]
        .into_iter()
        .map(|id| HostOperationRequirement {
            contract_id: HostOperationContractId::from(id),
            target_kind: Some(target_kind.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: if id == RHYTHM_DRAIN_HOST_OPERATION {
                0
            } else {
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
            },
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        })
        .collect(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: conduit_semantic_catalog::RHYTHM_MAXIMUM_PENDING_BEATS,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES
                * usize::from(conduit_semantic_catalog::RHYTHM_MAXIMUM_PENDING_BEATS)
                * 3) as u32,
        },
    }
}

pub fn instrument_map_std_offer() -> CapabilityOffer {
    let definition = conduit_semantic_catalog::instrument_map_definition()
        .expect("portable instrument-map definition is finite");
    CapabilityOffer {
        startup_parameters: vec![startup(
            "mapping",
            conduit_semantic_catalog::INSTRUMENT_MAPPING_TYPE,
            false,
        )],
        shorthand: None,
        capability_id: CapabilityId::from("music-instrument-map"),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        implementation: implementation(
            INSTRUMENT_MAP_STD_PROFILE,
            INSTRUMENT_MAP_STD_IMPLEMENTATION,
            INSTRUMENT_MAP_STD_ARTIFACT,
        ),
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 16,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}

fn implementation(profile: &str, implementation: &str, artifact: &str) -> ImplementationOffer {
    ImplementationOffer {
        execution_profile_id: ExecutionProfileId::from(profile),
        implementation_id: ImplementationId::from(implementation),
        artifact_id: ArtifactId::from(artifact),
    }
}

fn startup(name: &str, value_type: &str, has_default: bool) -> FaceStartupParameter {
    FaceStartupParameter {
        name: name.into(),
        value_type: value_type.into(),
        has_default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_offers_preserve_portable_faces_and_exact_effects() {
        let midi = music_play_midi_offer();
        assert_eq!(
            midi.inputs,
            conduit_semantic_catalog::music_play_contract().inputs
        );
        assert_eq!(midi.host_operations.len(), 2);
        assert_eq!(midi.authority_requirements.len(), 2);
        let input = music_input_midi_offer();
        assert_eq!(
            input.outputs,
            conduit_semantic_catalog::music_input_contract().outputs
        );
        assert_eq!(input.resource_requirements.len(), 1);
        assert_eq!(input.authority_requirements.len(), 1);
        let audio = audio_play_alsa_hw_offer();
        assert_eq!(
            audio.host_operations[0].maximum_input_bytes,
            conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES
        );
        assert_eq!(audio.resource_requirements.len(), 1);
        assert_eq!(audio.authority_requirements.len(), 1);
        let synth = music_synth_reference_offer();
        assert_eq!(
            synth.outputs,
            conduit_semantic_catalog::music_synth_contract().outputs
        );
    }
}
