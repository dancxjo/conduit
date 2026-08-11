//! Exact highest-honest-seam compatibility facts.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    PcmChannelLayout, PcmSampleRepresentation, RealizationCharacteristic,
    RealizationCharacteristicId, RealizationCharacteristicValue,
};
use serde::{Deserialize, Serialize};

pub const SOUND_SEAM_CHARACTERISTIC: &str = "sound/seam@1";
pub const SOUND_PROFILE_ID_CHARACTERISTIC: &str = "sound/profile-id@1";
pub const SOUND_MINIMUM_PITCH_CHARACTERISTIC: &str = "sound/minimum-pitch-millihertz@1";
pub const SOUND_MAXIMUM_PITCH_CHARACTERISTIC: &str = "sound/maximum-pitch-millihertz@1";
pub const MUSIC_MAXIMUM_POLYPHONY_CHARACTERISTIC: &str = "music/maximum-polyphony@1";
pub const MUSIC_MAXIMUM_EVENT_RATE_CHARACTERISTIC: &str = "music/maximum-events-per-second@1";
pub const MUSIC_VELOCITY_CHARACTERISTIC: &str = "music/preserves-velocity@1";
pub const MUSIC_SUSTAIN_CHARACTERISTIC: &str = "music/preserves-sustain@1";
pub const MUSIC_PITCH_BEND_CHARACTERISTIC: &str = "music/preserves-pitch-bend@1";
pub const MUSIC_MAXIMUM_PITCH_BEND_RANGE_CHARACTERISTIC: &str =
    "music/maximum-pitch-bend-range-microcents@1";
pub const MUSIC_MODULATION_CHARACTERISTIC: &str = "music/preserves-modulation@1";
pub const MUSIC_MICROTONAL_CHARACTERISTIC: &str = "music/accepts-microtonal-pitch@1";
pub const MUSIC_SUBTRACTIVE_FILTER_CHARACTERISTIC: &str = "music/subtractive-filter@1";
pub const AUDIO_SAMPLE_REPRESENTATION_CHARACTERISTIC: &str = "audio/sample-representation@1";
pub const AUDIO_SAMPLE_RATE_CHARACTERISTIC: &str = "audio/sample-rate-hz@1";
pub const AUDIO_CHANNEL_LAYOUT_CHARACTERISTIC: &str = "audio/channel-layout@1";
pub const AUDIO_MAXIMUM_FRAMES_CHARACTERISTIC: &str = "audio/maximum-frames-per-block@1";
pub const AUDIO_MAXIMUM_FRAME_BYTES_CHARACTERISTIC: &str = "audio/maximum-frame-bytes@1";
pub const AUDIO_PERIOD_FRAMES_CHARACTERISTIC: &str = "audio/period-frames@1";
pub const AUDIO_BUFFER_FRAMES_CHARACTERISTIC: &str = "audio/buffer-frames@1";
pub const AUDIO_MAXIMUM_BLOCKS_CHARACTERISTIC: &str = "audio/maximum-blocks-per-play@1";
pub const AUDIO_SOURCE_CLOCK_ID_CHARACTERISTIC: &str = "audio/source-clock-id@1";
pub const AUDIO_DEVICE_CLOCK_CHARACTERISTIC: &str = "audio/device-clock@1";
pub const AUDIO_PLAYBACK_RESOURCE_CHARACTERISTIC: &str = "audio/playback-resource@1";
pub const AUDIO_BACKEND_CHARACTERISTIC: &str = "audio/backend@1";
pub const AUDIO_STARTUP_POLICY_CHARACTERISTIC: &str = "audio/startup-policy@1";
pub const AUDIO_DRAIN_POLICY_CHARACTERISTIC: &str = "audio/drain-policy@1";
pub const AUDIO_TIMING_CLASS_CHARACTERISTIC: &str = "audio/timing-class@1";
pub const AUDIO_CONTROLLED_STAGING_BYTES_CHARACTERISTIC: &str = "audio/controlled-staging-bytes@1";

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoundSeam {
    Tone,
    MusicalEvents,
    Synthesis,
    PcmPlayback,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PcmCompatibilityProfile {
    pub representation: PcmSampleRepresentation,
    pub sample_rate_hz: u32,
    pub layout: PcmChannelLayout,
    pub maximum_frames_per_block: u16,
    pub maximum_frame_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundCompatibilityProfile {
    pub profile_id: String,
    pub seam: SoundSeam,
    pub minimum_pitch_millihertz: u64,
    pub maximum_pitch_millihertz: u64,
    pub maximum_polyphony: u16,
    pub maximum_events_per_second: u32,
    pub preserves_velocity: bool,
    pub preserves_sustain: bool,
    pub preserves_pitch_bend: bool,
    pub maximum_pitch_bend_range_microcents: u32,
    pub preserves_modulation: bool,
    pub accepts_microtonal_pitch: bool,
    pub supports_subtractive_filter: bool,
    pub pcm: Option<PcmCompatibilityProfile>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncompatibilityReason {
    WrongSemanticSeam,
    PitchRangeUnsupported,
    PolyphonyExceedsOffer,
    EventRateExceedsOffer,
    VelocityUnsupported,
    SustainUnsupported,
    PitchBendUnsupported,
    PitchBendRangeExceedsOffer,
    ModulationUnsupported,
    MicrotonalPitchUnsupported,
    SubtractiveFilterUnsupported,
    PcmProfileMissing,
    PcmRepresentationMismatch,
    PcmSampleRateMismatch,
    PcmLayoutMismatch,
    PcmBlockExceedsOffer,
}

pub fn compatibility(
    required: &SoundCompatibilityProfile,
    offered: &SoundCompatibilityProfile,
) -> Result<(), IncompatibilityReason> {
    if required.seam != offered.seam {
        return Err(IncompatibilityReason::WrongSemanticSeam);
    }
    if required.minimum_pitch_millihertz < offered.minimum_pitch_millihertz
        || required.maximum_pitch_millihertz > offered.maximum_pitch_millihertz
    {
        return Err(IncompatibilityReason::PitchRangeUnsupported);
    }
    if required.maximum_polyphony > offered.maximum_polyphony {
        return Err(IncompatibilityReason::PolyphonyExceedsOffer);
    }
    if required.maximum_events_per_second > offered.maximum_events_per_second {
        return Err(IncompatibilityReason::EventRateExceedsOffer);
    }
    if required.preserves_velocity && !offered.preserves_velocity {
        return Err(IncompatibilityReason::VelocityUnsupported);
    }
    if required.preserves_sustain && !offered.preserves_sustain {
        return Err(IncompatibilityReason::SustainUnsupported);
    }
    if required.preserves_pitch_bend && !offered.preserves_pitch_bend {
        return Err(IncompatibilityReason::PitchBendUnsupported);
    }
    if required.maximum_pitch_bend_range_microcents > offered.maximum_pitch_bend_range_microcents {
        return Err(IncompatibilityReason::PitchBendRangeExceedsOffer);
    }
    if required.preserves_modulation && !offered.preserves_modulation {
        return Err(IncompatibilityReason::ModulationUnsupported);
    }
    if required.accepts_microtonal_pitch && !offered.accepts_microtonal_pitch {
        return Err(IncompatibilityReason::MicrotonalPitchUnsupported);
    }
    if required.supports_subtractive_filter && !offered.supports_subtractive_filter {
        return Err(IncompatibilityReason::SubtractiveFilterUnsupported);
    }
    match (required.pcm, offered.pcm) {
        (None, _) => Ok(()),
        (Some(_), None) => Err(IncompatibilityReason::PcmProfileMissing),
        (Some(required), Some(offered)) => {
            if required.representation != offered.representation {
                return Err(IncompatibilityReason::PcmRepresentationMismatch);
            }
            if required.sample_rate_hz != offered.sample_rate_hz {
                return Err(IncompatibilityReason::PcmSampleRateMismatch);
            }
            if required.layout != offered.layout {
                return Err(IncompatibilityReason::PcmLayoutMismatch);
            }
            if required.maximum_frames_per_block > offered.maximum_frames_per_block
                || required.maximum_frame_bytes > offered.maximum_frame_bytes
            {
                return Err(IncompatibilityReason::PcmBlockExceedsOffer);
            }
            Ok(())
        }
    }
}

/// Converts a sound profile into the repository's canonical planner facts.
/// These facts are sealed into the selected `PlannedGear`; they are not an
/// alternate source of planning truth.
pub fn sound_profile_characteristics(
    profile: &SoundCompatibilityProfile,
) -> Vec<RealizationCharacteristic> {
    let mut facts = vec![
        label(SOUND_PROFILE_ID_CHARACTERISTIC, &profile.profile_id),
        label(SOUND_SEAM_CHARACTERISTIC, seam_label(profile.seam)),
        count(
            SOUND_MINIMUM_PITCH_CHARACTERISTIC,
            profile.minimum_pitch_millihertz,
        ),
        count(
            SOUND_MAXIMUM_PITCH_CHARACTERISTIC,
            profile.maximum_pitch_millihertz,
        ),
        count(
            MUSIC_MAXIMUM_POLYPHONY_CHARACTERISTIC,
            u64::from(profile.maximum_polyphony),
        ),
        count(
            MUSIC_MAXIMUM_EVENT_RATE_CHARACTERISTIC,
            u64::from(profile.maximum_events_per_second),
        ),
        flag(MUSIC_VELOCITY_CHARACTERISTIC, profile.preserves_velocity),
        flag(MUSIC_SUSTAIN_CHARACTERISTIC, profile.preserves_sustain),
        flag(
            MUSIC_PITCH_BEND_CHARACTERISTIC,
            profile.preserves_pitch_bend,
        ),
        count(
            MUSIC_MAXIMUM_PITCH_BEND_RANGE_CHARACTERISTIC,
            u64::from(profile.maximum_pitch_bend_range_microcents),
        ),
        flag(
            MUSIC_MODULATION_CHARACTERISTIC,
            profile.preserves_modulation,
        ),
        flag(
            MUSIC_MICROTONAL_CHARACTERISTIC,
            profile.accepts_microtonal_pitch,
        ),
        flag(
            MUSIC_SUBTRACTIVE_FILTER_CHARACTERISTIC,
            profile.supports_subtractive_filter,
        ),
    ];
    if let Some(pcm) = profile.pcm {
        facts.extend([
            label(
                AUDIO_SAMPLE_REPRESENTATION_CHARACTERISTIC,
                representation_label(pcm.representation),
            ),
            count(
                AUDIO_SAMPLE_RATE_CHARACTERISTIC,
                u64::from(pcm.sample_rate_hz),
            ),
            label(
                AUDIO_CHANNEL_LAYOUT_CHARACTERISTIC,
                layout_label(pcm.layout),
            ),
            count(
                AUDIO_MAXIMUM_FRAMES_CHARACTERISTIC,
                u64::from(pcm.maximum_frames_per_block),
            ),
            count(
                AUDIO_MAXIMUM_FRAME_BYTES_CHARACTERISTIC,
                u64::from(pcm.maximum_frame_bytes),
            ),
        ]);
    }
    facts.sort();
    facts
}

fn count(id: &str, value: u64) -> RealizationCharacteristic {
    characteristic(id, RealizationCharacteristicValue::Count(value))
}

fn flag(id: &str, value: bool) -> RealizationCharacteristic {
    characteristic(id, RealizationCharacteristicValue::Flag(value))
}

fn label(id: &str, value: &str) -> RealizationCharacteristic {
    characteristic(id, RealizationCharacteristicValue::Label(value.into()))
}

fn characteristic(id: &str, value: RealizationCharacteristicValue) -> RealizationCharacteristic {
    RealizationCharacteristic {
        characteristic_id: RealizationCharacteristicId::from(id),
        value,
    }
}

const fn seam_label(seam: SoundSeam) -> &'static str {
    match seam {
        SoundSeam::Tone => "tone",
        SoundSeam::MusicalEvents => "musical-events",
        SoundSeam::Synthesis => "synthesis",
        SoundSeam::PcmPlayback => "pcm-playback",
    }
}

const fn representation_label(representation: PcmSampleRepresentation) -> &'static str {
    match representation {
        PcmSampleRepresentation::Signed16LittleEndian => "signed-16-le",
        PcmSampleRepresentation::Signed24LittleEndian => "signed-24-le",
        PcmSampleRepresentation::Float32LittleEndian => "float-32-le",
    }
}

const fn layout_label(layout: PcmChannelLayout) -> &'static str {
    match layout {
        PcmChannelLayout::Mono => "mono",
        PcmChannelLayout::StereoLeftRight => "stereo-left-right",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str, seam: SoundSeam) -> SoundCompatibilityProfile {
        SoundCompatibilityProfile {
            profile_id: id.into(),
            seam,
            minimum_pitch_millihertz: 8_000,
            maximum_pitch_millihertz: 40_000_000,
            maximum_polyphony: 1,
            maximum_events_per_second: 1_000,
            preserves_velocity: false,
            preserves_sustain: false,
            preserves_pitch_bend: false,
            maximum_pitch_bend_range_microcents: 0,
            preserves_modulation: false,
            accepts_microtonal_pitch: true,
            supports_subtractive_filter: false,
            pcm: None,
        }
    }

    #[test]
    fn wrong_seam_and_implicit_loss_refuse_specifically() {
        let tone = profile("tone", SoundSeam::Tone);
        assert_eq!(compatibility(&tone, &tone), Ok(()));
        let mut music = profile("music", SoundSeam::MusicalEvents);
        music.maximum_polyphony = 8;
        music.preserves_velocity = true;
        music.preserves_pitch_bend = true;
        music.maximum_pitch_bend_range_microcents = 200_000_000;
        assert_eq!(
            compatibility(&music, &tone),
            Err(IncompatibilityReason::WrongSemanticSeam)
        );
        let mut limited = music.clone();
        limited.maximum_polyphony = 1;
        assert_eq!(
            compatibility(&music, &limited),
            Err(IncompatibilityReason::PolyphonyExceedsOffer)
        );
        limited.maximum_polyphony = 8;
        limited.maximum_events_per_second = 10;
        assert_eq!(
            compatibility(&music, &limited),
            Err(IncompatibilityReason::EventRateExceedsOffer)
        );
        limited.maximum_events_per_second = 1_000;
        limited.preserves_pitch_bend = true;
        limited.maximum_pitch_bend_range_microcents = 100_000_000;
        assert_eq!(
            compatibility(&music, &limited),
            Err(IncompatibilityReason::PitchBendRangeExceedsOffer)
        );
        limited.maximum_pitch_bend_range_microcents = 200_000_000;
        limited.accepts_microtonal_pitch = false;
        assert_eq!(
            compatibility(&music, &limited),
            Err(IncompatibilityReason::MicrotonalPitchUnsupported)
        );
        let mut synth = music.clone();
        synth.supports_subtractive_filter = true;
        limited.accepts_microtonal_pitch = true;
        assert_eq!(
            compatibility(&synth, &limited),
            Err(IncompatibilityReason::SubtractiveFilterUnsupported)
        );
    }

    #[test]
    fn pcm_profile_is_exact_and_finitely_bounded() {
        let pcm = PcmCompatibilityProfile {
            representation: PcmSampleRepresentation::Signed16LittleEndian,
            sample_rate_hz: 48_000,
            layout: PcmChannelLayout::StereoLeftRight,
            maximum_frames_per_block: 256,
            maximum_frame_bytes: 1_024,
        };
        let mut required = profile("pcm-required", SoundSeam::PcmPlayback);
        required.pcm = Some(pcm);
        let mut offered = required.clone();
        assert_eq!(compatibility(&required, &offered), Ok(()));
        offered.pcm.as_mut().unwrap().sample_rate_hz = 44_100;
        assert_eq!(
            compatibility(&required, &offered),
            Err(IncompatibilityReason::PcmSampleRateMismatch)
        );
        offered.pcm = None;
        assert_eq!(
            compatibility(&required, &offered),
            Err(IncompatibilityReason::PcmProfileMissing)
        );
        offered.pcm = Some(PcmCompatibilityProfile {
            maximum_frames_per_block: 128,
            ..pcm
        });
        assert_eq!(
            compatibility(&required, &offered),
            Err(IncompatibilityReason::PcmBlockExceedsOffer)
        );

        let facts = sound_profile_characteristics(&required);
        assert!(facts.iter().any(|fact| {
            fact.characteristic_id.as_str() == AUDIO_SAMPLE_RATE_CHARACTERISTIC
                && fact.value == RealizationCharacteristicValue::Count(48_000)
        }));
        assert!(facts.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
