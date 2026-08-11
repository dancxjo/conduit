//! Host-neutral sound/music semantic waist.

use super::{StandardKindContract, TerminalBehavior};
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    AUDIO_PCM_INFO_ID, CONTROL_EVENT_ENCODED_LEN, MUSIC_CONTROL_INFO_ID, MUSIC_NOTE_INFO_ID,
    NOTE_EVENT_ENCODED_LEN, PCM_FRAME_HEADER_ENCODED_LEN, SOUND_TONE_INFO_ID,
};
use serde::{Deserialize, Serialize};

pub const SOUND_TONE_PLAY_KIND: &str = "sound/tone-play";
pub const MUSIC_PLAY_KIND: &str = "music/play";
pub const MUSIC_SYNTH_KIND: &str = "music/synth";
pub const AUDIO_PLAY_KIND: &str = "audio/play";
pub const SOUND_TONE_PLAY_REVISION: &str = "conduit.std/sound-tone-play@1";
pub const MUSIC_PLAY_REVISION: &str = "conduit.std/music-play@1";
pub const MUSIC_SYNTH_REVISION: &str = "conduit.std/music-synth@1";
pub const MUSIC_SYNTH_REFERENCE_PROFILE: &str = "conduit.reference/music-synth-fixed-q16@1";
pub const MUSIC_SYNTH_REFERENCE_IMPLEMENTATION: &str = "std/kernel-music-synth-fixed-q16@1";
pub const MUSIC_SYNTH_REFERENCE_ARTIFACT: &str = "conduit-std-host/music-synth-fixed-q16@1";
pub const MUSIC_SYNTH_HOST_OPERATION: &str = "conduit.host/music-synth-render-fixed-q16@1";
pub const MUSIC_SYNTH_PCM_BLOCK_BYTES: u32 = PCM_FRAME_HEADER_ENCODED_LEN as u32 + 256 * 2;
pub const AUDIO_PLAY_REVISION: &str = "conduit.std/audio-play@1";
pub const AUDIO_PLAY_ALSA_HW_PROFILE: &str = "std/alsa-hw-s16le-48000-stereo-p256-b1024@1";
pub const AUDIO_PLAY_ALSA_HW_IMPLEMENTATION: &str = "std/kernel-audio-play-alsa-hw@1";
pub const AUDIO_PLAY_ALSA_HW_ARTIFACT: &str = "conduit-std-host/alsa-aplay-hw@1";
pub const AUDIO_PLAY_ALSA_HW_OPERATION: &str = "conduit.host/audio-play-alsa-hw@1";
pub const AUDIO_PLAYBACK_RESOURCE_CLASS: &str = "conduit.resource/audio-playback-alsa-hw@1";
pub const AUDIO_PLAYBACK_AUTHORITY_CONTRACT: &str = "conduit.authority/audio-playback@1";
pub const AUDIO_PLAY_ALSA_PERIOD_FRAMES: u16 = 256;
pub const AUDIO_PLAY_ALSA_BUFFER_FRAMES: u16 = 1_024;
pub const AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS: u16 = 256;
pub const AUDIO_PLAY_ALSA_FRAME_BYTES: u32 = 4;
pub const AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES: u32 = PCM_FRAME_HEADER_ENCODED_LEN as u32
    + AUDIO_PLAY_ALSA_PERIOD_FRAMES as u32 * AUDIO_PLAY_ALSA_FRAME_BYTES;

pub const MAXIMUM_MUSICAL_EVENT_ITEMS: u16 = 256;
pub const MAXIMUM_MUSICAL_EVENT_BYTES: u32 = 16_384;
pub const MAXIMUM_SIMULTANEOUS_NOTES: u16 = 64;
pub const MAXIMUM_AUDIO_QUEUE_ITEMS: u16 = 8;
pub const MAXIMUM_AUDIO_QUEUE_BYTES: u32 = 524_288;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressureDisposition {
    WaitWithoutConsumption,
    RefuseBeforePlay,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancellationDisposition {
    CancelAndReleaseFiniteState,
    DrainThenComplete,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoundTerminalBehavior {
    CompletesWhenInputsClose,
    DrainsAdmittedOutputThenCompletes,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamSemantics {
    pub maximum_queue_items: u16,
    pub maximum_queue_bytes: u32,
    pub maximum_outstanding_notes: u16,
    pub pressure: PressureDisposition,
    pub cancellation: CancellationDisposition,
    pub terminal: SoundTerminalBehavior,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardRealizationForm {
    pub requirement_kind: &'static str,
    pub stages: [&'static str; 2],
}

/// Ordinary reusable expansion; planners need no backend-specific switch.
pub const MUSIC_PLAY_THROUGH_SYNTH: StandardRealizationForm = StandardRealizationForm {
    requirement_kind: MUSIC_PLAY_KIND,
    stages: [MUSIC_SYNTH_KIND, AUDIO_PLAY_KIND],
};

pub fn sound_tone_play_contract() -> StandardKindContract {
    sink(
        SOUND_TONE_PLAY_KIND,
        "Play tone",
        "Consume bounded portable pitch/gate intent.",
        vec![port("tone", SOUND_TONE_INFO_ID, PortDirection::Input)],
        tone_limits(),
    )
}

pub fn music_play_contract() -> StandardKindContract {
    sink(
        MUSIC_PLAY_KIND,
        "Play music",
        "Consume portable note and typed expressive-control events.",
        music_inputs(),
        event_limits(),
    )
}

pub fn music_synth_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(MUSIC_SYNTH_KIND),
        plain_name: "Synthesize music".to_string(),
        summary: "Transform portable musical events into bounded timestamped PCM frames."
            .to_string(),
        inputs: music_inputs(),
        outputs: vec![port("audio", AUDIO_PCM_INFO_ID, PortDirection::Output)],
        configuration: Vec::new(),
        limits: audio_limits(),
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "synth: music/synth".to_string(),
    }
}

pub fn music_synth_reference_offer() -> CapabilityOffer {
    let contract = music_synth_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("music-synth-fixed-q16"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(MUSIC_SYNTH_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(MUSIC_SYNTH_REFERENCE_PROFILE),
            implementation_id: ImplementationId::from(MUSIC_SYNTH_REFERENCE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(MUSIC_SYNTH_REFERENCE_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(MUSIC_SYNTH_HOST_OPERATION),
            target_kind: Some(kind_id(AUDIO_PCM_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: NOTE_EVENT_ENCODED_LEN.max(CONTROL_EVENT_ENCODED_LEN) as u32,
            maximum_output_bytes: MUSIC_SYNTH_PCM_BLOCK_BYTES,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: audio_limits(),
    }
}

pub fn audio_play_contract() -> StandardKindContract {
    sink(
        AUDIO_PLAY_KIND,
        "Play audio",
        "Consume bounded timestamped PCM through an exact selected playback resource.",
        vec![port("audio", AUDIO_PCM_INFO_ID, PortDirection::Input)],
        audio_limits(),
    )
}

/// Exact direct-ALSA playback implementation. The containing Host
/// advertisement must contribute one freshly observed playback resource; the
/// offer alone neither discovers nor authorizes a device.
pub fn audio_play_alsa_hw_offer() -> CapabilityOffer {
    let contract = audio_play_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("audio-play-alsa-hw"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(AUDIO_PLAY_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(AUDIO_PLAY_ALSA_HW_PROFILE),
            implementation_id: ImplementationId::from(AUDIO_PLAY_ALSA_HW_IMPLEMENTATION),
            artifact_id: ArtifactId::from(AUDIO_PLAY_ALSA_HW_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(AUDIO_PLAY_ALSA_HW_OPERATION),
            target_kind: Some(kind_id(AUDIO_PCM_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![conduit_core::resource_requirement(
            AUDIO_PLAYBACK_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: vec![conduit_core::AuthorityRequirement {
            contract_id: conduit_core::AuthorityContractId::from(AUDIO_PLAYBACK_AUTHORITY_CONTRACT),
            host_operation_contract_id: HostOperationContractId::from(AUDIO_PLAY_ALSA_HW_OPERATION),
            subject_kind: kind_id(AUDIO_PCM_INFO_ID),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_AUDIO_QUEUE_ITEMS,
            max_queue_bytes: MAXIMUM_AUDIO_QUEUE_BYTES,
        },
    }
}

pub fn sound_contracts_with_revisions() -> [(StandardKindContract, &'static str); 4] {
    [
        (sound_tone_play_contract(), SOUND_TONE_PLAY_REVISION),
        (music_play_contract(), MUSIC_PLAY_REVISION),
        (music_synth_contract(), MUSIC_SYNTH_REVISION),
        (audio_play_contract(), AUDIO_PLAY_REVISION),
    ]
}

pub fn stream_semantics(kind: &str) -> Option<StreamSemantics> {
    let (items, bytes, notes, cancellation, terminal) = match kind {
        SOUND_TONE_PLAY_KIND => (
            MAXIMUM_MUSICAL_EVENT_ITEMS,
            MAXIMUM_MUSICAL_EVENT_BYTES,
            1,
            CancellationDisposition::CancelAndReleaseFiniteState,
            SoundTerminalBehavior::CompletesWhenInputsClose,
        ),
        MUSIC_PLAY_KIND | MUSIC_SYNTH_KIND => (
            MAXIMUM_MUSICAL_EVENT_ITEMS,
            MAXIMUM_MUSICAL_EVENT_BYTES,
            MAXIMUM_SIMULTANEOUS_NOTES,
            CancellationDisposition::CancelAndReleaseFiniteState,
            SoundTerminalBehavior::CompletesWhenInputsClose,
        ),
        AUDIO_PLAY_KIND => (
            MAXIMUM_AUDIO_QUEUE_ITEMS,
            MAXIMUM_AUDIO_QUEUE_BYTES,
            0,
            CancellationDisposition::DrainThenComplete,
            SoundTerminalBehavior::DrainsAdmittedOutputThenCompletes,
        ),
        _ => return None,
    };
    Some(StreamSemantics {
        maximum_queue_items: items,
        maximum_queue_bytes: bytes,
        maximum_outstanding_notes: notes,
        pressure: PressureDisposition::WaitWithoutConsumption,
        cancellation,
        terminal,
    })
}

fn sink(
    kind: &str,
    name: &str,
    summary: &str,
    inputs: Vec<PortDescriptor>,
    limits: CapabilityLimits,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: name.to_string(),
        summary: summary.to_string(),
        inputs,
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits,
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: alloc::format!("output: {kind}"),
    }
}

fn music_inputs() -> Vec<PortDescriptor> {
    vec![
        port("notes", MUSIC_NOTE_INFO_ID, PortDirection::Input),
        port("controls", MUSIC_CONTROL_INFO_ID, PortDirection::Input),
    ]
}
fn port(name: &str, info: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal: PortTemporal::Value,
    }
}
fn tone_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: MAXIMUM_MUSICAL_EVENT_ITEMS,
        max_queue_bytes: MAXIMUM_MUSICAL_EVENT_BYTES,
    }
}
fn event_limits() -> CapabilityLimits {
    tone_limits()
}
fn audio_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: MAXIMUM_AUDIO_QUEUE_ITEMS,
        max_queue_bytes: MAXIMUM_AUDIO_QUEUE_BYTES,
    }
}

pub fn sound_contract_revision(kind: &str) -> Option<KindContractRevision> {
    sound_contracts_with_revisions()
        .into_iter()
        .find(|(contract, _)| contract.kind_id.as_str() == kind)
        .map(|(_, revision)| KindContractRevision::from(revision))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_faces_are_distinct_and_backend_free() {
        let encoded = alloc::format!("{:?}", sound_contracts_with_revisions());
        for forbidden in [
            "MIDI",
            "ALSA",
            "PipeWire",
            "OPL",
            "Create",
            "device-name",
            "default-output",
        ] {
            assert!(
                !encoded.contains(forbidden),
                "portable catalog contains {forbidden}"
            );
        }
        assert_ne!(music_play_contract().inputs, audio_play_contract().inputs);
        assert_eq!(
            MUSIC_PLAY_THROUGH_SYNTH.stages,
            [MUSIC_SYNTH_KIND, AUDIO_PLAY_KIND]
        );
    }

    #[test]
    fn all_storage_and_pressure_are_finite() {
        for kind in [
            SOUND_TONE_PLAY_KIND,
            MUSIC_PLAY_KIND,
            MUSIC_SYNTH_KIND,
            AUDIO_PLAY_KIND,
        ] {
            let semantics = stream_semantics(kind).unwrap();
            assert!(semantics.maximum_queue_items > 0);
            assert!(semantics.maximum_queue_bytes > 0);
            assert_eq!(
                semantics.pressure,
                PressureDisposition::WaitWithoutConsumption
            );
        }
    }

    #[test]
    fn alsa_playback_offer_requires_resource_and_independent_authority() {
        let offer = audio_play_alsa_hw_offer();
        assert_eq!(offer.kind_id.as_str(), AUDIO_PLAY_KIND);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(offer.authority_requirements.len(), 1);
        assert_eq!(offer.host_operations.len(), 1);
        assert_eq!(offer.host_operations[0].maximum_in_flight, 1);
        assert_eq!(offer.host_operations[0].maximum_output_bytes, 0);
        assert_eq!(
            offer.host_operations[0].maximum_input_bytes,
            AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES
        );
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn synth_playback_realization_is_an_ordinary_recursive_form() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        crate::install_sound_catalogs(&mut startup, &mut profile).unwrap();
        for (kind, info) in [
            ("test/note-source", MUSIC_NOTE_INFO_ID),
            ("test/control-source", MUSIC_CONTROL_INFO_ID),
        ] {
            startup
                .insert(conduit_form::KindSignature {
                    kind: kind.into(),
                    startup_parameters: Vec::new(),
                })
                .unwrap();
            profile
                .insert(conduit_form::KindDefinition {
                    kind_id: kind_id(kind),
                    kind_contract_revision: KindContractRevision::from(alloc::format!("{kind}@1")),
                    inputs: Vec::new(),
                    outputs: vec![port("out", info, PortDirection::Output)],
                    configuration: Vec::new(),
                })
                .unwrap();
        }
        let source = "form music/play-through-synth (\n > notes: music/note-event@1\n > controls: music/control-event@1\n) {\n synth: music/synth\n output: audio/play\n notes > synth.notes\n controls > synth.controls\n synth.audio > output.audio\n}\n\nform instrument-output {\n notes: test/note-source\n controls: test/control-source\n realization: music/play-through-synth\n notes > realization.notes\n controls > realization.controls\n}\n";
        let syntax = conduit_form::parse_syntax_document(source);
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        let expanded =
            conduit_form::expand_canonical_form(&checked, "instrument-output", &profile).unwrap();
        assert_eq!(expanded.gears.len(), 4);
        assert!(expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == MUSIC_SYNTH_KIND));
        assert!(expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == AUDIO_PLAY_KIND));
        assert_eq!(expanded.connections.len(), 3);
        expanded.validate_expansion().unwrap();
    }
}
