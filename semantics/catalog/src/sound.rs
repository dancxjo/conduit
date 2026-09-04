//! Host-neutral sound/music semantic waist.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use crate::{
    audio_render_demand_contract, music_input_contract, AUDIO_RENDER_DEMAND_REVISION,
    MUSIC_INPUT_KIND, MUSIC_INPUT_REVISION,
};
use alloc::string::{String, ToString};
use alloc::{vec, vec::Vec};
use conduit_audio::{
    AUDIO_PCM_INFO_ID, AUDIO_RENDER_DEMAND_INFO_ID, MUSIC_CONTROL_INFO_ID, MUSIC_NOTE_INFO_ID,
    PCM_FRAME_HEADER_ENCODED_LEN, SOUND_TONE_INFO_ID,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal,
};
use serde::{Deserialize, Serialize};

pub const SOUND_TONE_PLAY_KIND: &str = "sound/tone-play";
pub const MUSIC_PLAY_KIND: &str = "music/play";
pub const MUSIC_SYNTH_KIND: &str = "music/synth";
pub const AUDIO_PLAY_KIND: &str = "audio/play";
pub const SOUND_TONE_PLAY_REVISION: &str = "conduit.std/sound-tone-play@1";
pub const MUSIC_PLAY_REVISION: &str = "conduit.std/music-play@1";
pub const MUSIC_SYNTH_REVISION: &str = "conduit.std/music-synth@1";
pub const MUSIC_SYNTH_PCM_BLOCK_BYTES: u32 = PCM_FRAME_HEADER_ENCODED_LEN as u32 + 256 * 4;
pub const SYNTH_MAXIMUM_VOICES_KEY: &str = "maximum-voices";
pub const SYNTH_OSCILLATOR_KEY: &str = "oscillator";
pub const SYNTH_PULSE_WIDTH_KEY: &str = "pulse-width-q16";
pub const SYNTH_ATTACK_KEY: &str = "attack-micros";
pub const SYNTH_DECAY_KEY: &str = "decay-micros";
pub const SYNTH_SUSTAIN_KEY: &str = "sustain-level-q16";
pub const SYNTH_RELEASE_KEY: &str = "release-micros";
pub const SYNTH_FILTER_CUTOFF_KEY: &str = "filter-cutoff-q16";
pub const SYNTH_FILTER_RESONANCE_KEY: &str = "filter-resonance-q16";
pub const SYNTH_FILTER_ENVELOPE_KEY: &str = "filter-envelope-amount-q16";
pub const SYNTH_LFO_RATE_KEY: &str = "lfo-rate-millihertz";
pub const SYNTH_LFO_DEPTH_KEY: &str = "lfo-depth-q16";
pub const SYNTH_MASTER_GAIN_KEY: &str = "master-gain-q16";
pub const SYNTH_STEAL_POLICY_KEY: &str = "voice-steal-policy";
pub const AUDIO_PLAY_REVISION: &str = "conduit.std/audio-play@1";
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
        inputs: {
            let mut inputs = music_inputs();
            inputs.push(port(
                "render",
                AUDIO_RENDER_DEMAND_INFO_ID,
                PortDirection::Input,
            ));
            inputs
        },
        outputs: vec![port("audio", AUDIO_PCM_INFO_ID, PortDirection::Output)],
        configuration: music_synth_configuration(),
        limits: audio_limits(),
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "synth: music/synth".to_string(),
    }
}

pub fn music_synth_configuration() -> Vec<StandardConfigurationField> {
    vec![
        u64_configuration(SYNTH_MAXIMUM_VOICES_KEY, 8, 8, 16),
        text_one_of_configuration(
            SYNTH_OSCILLATOR_KEY,
            "saw",
            &["sine", "triangle", "saw", "pulse"],
        ),
        u64_configuration(SYNTH_PULSE_WIDTH_KEY, 32_768, 3_277, 62_259),
        u64_configuration(SYNTH_ATTACK_KEY, 10_000, 0, 30_000_000),
        u64_configuration(SYNTH_DECAY_KEY, 80_000, 0, 30_000_000),
        u64_configuration(SYNTH_SUSTAIN_KEY, 45_875, 0, 65_535),
        u64_configuration(SYNTH_RELEASE_KEY, 150_000, 0, 30_000_000),
        u64_configuration(SYNTH_FILTER_CUTOFF_KEY, 18_000, 1, 32_768),
        u64_configuration(SYNTH_FILTER_RESONANCE_KEY, 20_000, 0, 60_000),
        i64_configuration(SYNTH_FILTER_ENVELOPE_KEY, 12_000, -32_768, 32_768),
        u64_configuration(SYNTH_LFO_RATE_KEY, 5_000, 0, 20_000),
        u64_configuration(SYNTH_LFO_DEPTH_KEY, 2_000, 0, 65_535),
        u64_configuration(SYNTH_MASTER_GAIN_KEY, 16_384, 0, 65_535),
        text_one_of_configuration(
            SYNTH_STEAL_POLICY_KEY,
            "oldest-released-then-oldest-active",
            &["oldest-released-then-oldest-active", "refuse"],
        ),
    ]
}

fn u64_configuration(
    key: &str,
    default_value: u64,
    minimum: u64,
    maximum: u64,
) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(default_value),
        rule: StandardConfigurationRule::U64Range { minimum, maximum },
    }
}

fn i64_configuration(
    key: &str,
    default_value: i64,
    minimum: i64,
    maximum: i64,
) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::I64(default_value),
        rule: StandardConfigurationRule::I64Range { minimum, maximum },
    }
}

fn text_one_of_configuration(
    key: &str,
    default_value: &str,
    values: &[&str],
) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text(default_value.into()),
        rule: StandardConfigurationRule::TextOneOf {
            values: values.iter().map(|value| String::from(*value)).collect(),
        },
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

pub fn sound_contracts_with_revisions() -> [(StandardKindContract, &'static str); 6] {
    [
        (sound_tone_play_contract(), SOUND_TONE_PLAY_REVISION),
        (music_input_contract(), MUSIC_INPUT_REVISION),
        (music_play_contract(), MUSIC_PLAY_REVISION),
        (music_synth_contract(), MUSIC_SYNTH_REVISION),
        (audio_render_demand_contract(), AUDIO_RENDER_DEMAND_REVISION),
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
        MUSIC_INPUT_KIND | MUSIC_PLAY_KIND | MUSIC_SYNTH_KIND => (
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
    music_ports(PortDirection::Input)
}
pub(crate) fn music_ports(direction: PortDirection) -> Vec<PortDescriptor> {
    vec![
        port("notes", MUSIC_NOTE_INFO_ID, direction),
        port("controls", MUSIC_CONTROL_INFO_ID, direction),
    ]
}
pub(super) fn port(name: &str, info: &str, direction: PortDirection) -> PortDescriptor {
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
pub(crate) fn event_limits() -> CapabilityLimits {
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
mod tests;
