use crate::{OscillatorShape, REFERENCE_MAXIMUM_BLOCK_FRAMES, REFERENCE_SAMPLE_RATE_HZ};

pub const UNITY_Q16: u32 = 65_536;
pub const MAXIMUM_ENVELOPE_MICROS: u32 = 30_000_000;
pub const MAXIMUM_LFO_MILLIHERTZ: u32 = 20_000;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum VoiceStealPolicy {
    OldestReleasedThenOldestActive,
    Refuse,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SynthProfileError {
    VoiceLimit,
    BlockFrames,
    EnvelopeTime,
    SustainLevel,
    FilterCutoff,
    FilterResonance,
    FilterEnvelopeAmount,
    LfoRate,
    LfoDepth,
    MasterGain,
    PulseWidth,
}

/// Immutable exact synthesis facts sealed into one Plan.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ReferenceSynthProfile {
    pub maximum_voices: u8,
    pub maximum_block_frames: u16,
    pub oscillator: OscillatorShape,
    pub pulse_width_q16: u16,
    pub attack_micros: u32,
    pub decay_micros: u32,
    pub sustain_level_q16: u16,
    pub release_micros: u32,
    pub filter_cutoff_q16: u16,
    pub filter_resonance_q16: u16,
    pub filter_envelope_amount_q16: i16,
    pub lfo_rate_millihertz: u32,
    pub lfo_depth_q16: u16,
    pub master_gain_q16: u16,
    pub steal_policy: VoiceStealPolicy,
}

impl ReferenceSynthProfile {
    pub fn musician_reference() -> Self {
        Self {
            maximum_voices: 8,
            maximum_block_frames: REFERENCE_MAXIMUM_BLOCK_FRAMES,
            oscillator: OscillatorShape::Saw,
            pulse_width_q16: 32_768,
            attack_micros: 10_000,
            decay_micros: 80_000,
            sustain_level_q16: 45_875,
            release_micros: 150_000,
            filter_cutoff_q16: 18_000,
            filter_resonance_q16: 20_000,
            filter_envelope_amount_q16: 12_000,
            lfo_rate_millihertz: 5_000,
            lfo_depth_q16: 2_000,
            master_gain_q16: 16_384,
            steal_policy: VoiceStealPolicy::OldestReleasedThenOldestActive,
        }
    }

    pub fn validate(self) -> Result<Self, SynthProfileError> {
        if self.maximum_voices < 8
            || usize::from(self.maximum_voices) > crate::REFERENCE_MAXIMUM_VOICES
        {
            return Err(SynthProfileError::VoiceLimit);
        }
        if self.maximum_block_frames == 0
            || self.maximum_block_frames > REFERENCE_MAXIMUM_BLOCK_FRAMES
        {
            return Err(SynthProfileError::BlockFrames);
        }
        if [self.attack_micros, self.decay_micros, self.release_micros]
            .into_iter()
            .any(|value| value > MAXIMUM_ENVELOPE_MICROS)
        {
            return Err(SynthProfileError::EnvelopeTime);
        }
        if u32::from(self.sustain_level_q16) > UNITY_Q16 {
            return Err(SynthProfileError::SustainLevel);
        }
        if self.filter_cutoff_q16 == 0 || self.filter_cutoff_q16 > 32_768 {
            return Err(SynthProfileError::FilterCutoff);
        }
        if self.filter_resonance_q16 > 60_000 {
            return Err(SynthProfileError::FilterResonance);
        }
        if self.filter_envelope_amount_q16.unsigned_abs() > 32_768 {
            return Err(SynthProfileError::FilterEnvelopeAmount);
        }
        if self.lfo_rate_millihertz > MAXIMUM_LFO_MILLIHERTZ {
            return Err(SynthProfileError::LfoRate);
        }
        if u32::from(self.lfo_depth_q16) > UNITY_Q16 {
            return Err(SynthProfileError::LfoDepth);
        }
        if u32::from(self.master_gain_q16) > UNITY_Q16 {
            return Err(SynthProfileError::MasterGain);
        }
        if self.pulse_width_q16 < 3_277 || self.pulse_width_q16 > 62_259 {
            return Err(SynthProfileError::PulseWidth);
        }
        Ok(self)
    }

    pub const fn sample_rate_hz(self) -> u32 {
        REFERENCE_SAMPLE_RATE_HZ
    }

    pub const fn retained_state_bytes(self) -> u32 {
        core::mem::size_of::<crate::ReferenceSynth>() as u32
    }
}

pub(crate) const fn micros_to_samples(micros: u32) -> u32 {
    let product = micros as u64 * REFERENCE_SAMPLE_RATE_HZ as u64;
    let rounded_up = product.saturating_add(999_999) / 1_000_000;
    if rounded_up > u32::MAX as u64 {
        u32::MAX
    } else {
        rounded_up as u32
    }
}
