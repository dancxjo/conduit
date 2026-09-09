//! The original Conduit cue, rendered without devices or lifecycle policy.
//!
//! This is a bounded DSP score. Creating it does not imply a Body Wake or
//! permission to play audio. The caller owns admission, output pressure, and
//! cancellation; first-wake State must never be stored in this renderer.

use conduit_audio::{Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId};

use crate::{OscillatorShape, ReferenceSynth, ReferenceSynthProfile, VoiceStealPolicy};

pub const STARTUP_CHIME_SCORE_ID: &str = "conduit.sound/startup-chime@1";
pub const STARTUP_CHIME_DURATION_MICROS: u32 = 1_200_000;
pub const STARTUP_CHIME_FRAMES: u32 = 57_600;

/// Musical meaning: three softly overlapping tones, each with an explicit
/// start, gate duration, pitch, and strength. No device or Boot facts occur.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct StartupChimeTone {
    pub frequency_millihertz: u64,
    pub start_micros: u32,
    pub gate_micros: u32,
    pub velocity: u16,
}

pub const STARTUP_CHIME_TONES: [StartupChimeTone; 3] = [
    StartupChimeTone {
        frequency_millihertz: 294_000,
        start_micros: 0,
        gate_micros: 380_000,
        velocity: 36_000,
    },
    StartupChimeTone {
        frequency_millihertz: 441_000,
        start_micros: 120_000,
        gate_micros: 450_000,
        velocity: 30_000,
    },
    StartupChimeTone {
        frequency_millihertz: 588_000,
        start_micros: 270_000,
        gate_micros: 470_000,
        velocity: 27_000,
    },
];

/// Reuses the existing integer reference DSP. Each tone has its own voice
/// bank so the reference synth's active-voice normalization cannot abruptly
/// change an already sounding tone when a later tone begins or ends.
pub fn startup_chime_profile() -> ReferenceSynthProfile {
    ReferenceSynthProfile {
        maximum_voices: 8,
        maximum_block_frames: 256,
        oscillator: OscillatorShape::Triangle,
        pulse_width_q16: 32_768,
        attack_micros: 40_000,
        decay_micros: 160_000,
        sustain_level_q16: 24_000,
        release_micros: 340_000,
        filter_cutoff_q16: 20_000,
        filter_resonance_q16: 0,
        filter_envelope_amount_q16: 0,
        lfo_rate_millihertz: 0,
        lfo_depth_q16: 0,
        master_gain_q16: 24_000,
        steal_policy: VoiceStealPolicy::Refuse,
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StartupChimeRenderRefusal {
    EmptyBlock,
    BlockTooLarge,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct StartupChime {
    tones: [ReferenceSynth; 3],
    frame: u32,
    stopped: bool,
}

impl Default for StartupChime {
    fn default() -> Self {
        Self::new()
    }
}

impl StartupChime {
    pub fn new() -> Self {
        Self {
            tones: [ReferenceSynth::new(startup_chime_profile())
                .expect("reviewed startup cue profile"); 3],
            frame: 0,
            stopped: false,
        }
    }

    pub const fn frame_cursor(&self) -> u32 {
        self.frame
    }

    pub const fn is_finished(&self) -> bool {
        self.stopped || self.frame == STARTUP_CHIME_FRAMES
    }

    /// Writes at most 256 mono signed-16 frames at the reference 48 kHz.
    /// Returns the written prefix length, or zero after completion/cancel.
    /// A refused block leaves both renderer and output unchanged. Pressure
    /// requires the caller to retain a produced block before asking for more.
    pub fn render(&mut self, output: &mut [i16]) -> Result<usize, StartupChimeRenderRefusal> {
        if output.is_empty() {
            return Err(StartupChimeRenderRefusal::EmptyBlock);
        }
        if output.len() > 256 {
            return Err(StartupChimeRenderRefusal::BlockTooLarge);
        }
        if self.is_finished() {
            return Ok(0);
        }
        let count = output
            .len()
            .min((STARTUP_CHIME_FRAMES - self.frame) as usize);
        for sample in &mut output[..count] {
            let mut mixed = 0_i32;
            for (index, (synth, tone)) in self.tones.iter_mut().zip(STARTUP_CHIME_TONES).enumerate()
            {
                for (micros, gate, order) in [
                    (tone.start_micros, Gate::On, 0),
                    (tone.start_micros + tone.gate_micros, Gate::Off, 1),
                ] {
                    if self.frame == micros_to_frame(micros) {
                        let event = MusicalNoteEvent::new(
                            NoteOccurrenceId(index as u64 + 1),
                            MusicalPitch::new(tone.frequency_millihertz, 440_000, 0)
                                .expect("reviewed cue pitch"),
                            gate,
                            tone.velocity,
                            u64::from(micros),
                            order,
                        )
                        .expect("reviewed cue event");
                        synth.apply_note(event).expect("ordered cue event");
                    }
                }
                let mut frame = [0_i16];
                synth.render(&mut frame);
                mixed += i32::from(frame[0]);
            }
            // Fixed normalization preserves each tone's envelope across the
            // other tones' lifetime boundaries and cannot clip signed-16.
            *sample = (mixed / 3) as i16;
            self.frame += 1;
        }
        Ok(count)
    }

    pub fn cancel(&mut self) {
        for tone in &mut self.tones {
            tone.stop();
        }
        self.stopped = true;
    }
}

const fn micros_to_frame(micros: u32) -> u32 {
    (micros as u64 * crate::REFERENCE_SAMPLE_RATE_HZ as u64 / 1_000_000) as u32
}
