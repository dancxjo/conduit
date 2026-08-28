use conduit_audio::{
    Gate, ModulationDestination, MusicalControl, MusicalControlEvent, MusicalNoteEvent,
    NoteOccurrenceId,
};

use crate::oscillator::clamp_i64_to_i32;
use crate::{
    ReferenceSynthProfile, SynthProfileError, Voice, VoiceAllocationOutcome, VoiceStealPolicy,
    REFERENCE_MAXIMUM_VOICES, REFERENCE_SAMPLE_RATE_HZ,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SynthEventError {
    InvalidProfile(SynthProfileError),
    StaleEventTime,
    DuplicateOccurrence,
    UnknownOccurrence,
    DuplicateNoteOff,
    VoiceExhausted,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SynthEventOutcome {
    NoteOn(VoiceAllocationOutcome),
    NoteOff { slot: u8, sustained: bool },
    Sustain { down: bool, released_voices: u8 },
    PitchBend,
    Modulation,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RenderSummary {
    pub first_frame: u64,
    pub frame_count: u16,
    pub active_voices_before: u8,
    pub active_voices_after: u8,
    pub clipped_samples: u16,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ReferenceSynth {
    profile: ReferenceSynthProfile,
    voices: [Option<Voice>; REFERENCE_MAXIMUM_VOICES],
    frame_cursor: u64,
    allocation_order: u64,
    sustain_down: bool,
    pitch_bend_amount_millionths: i32,
    pitch_bend_range_microcents: u32,
    modulation_amount_millionths: u32,
    modulation_destination: ModulationDestination,
    lfo_phase: u32,
}

impl ReferenceSynth {
    pub fn new(profile: ReferenceSynthProfile) -> Result<Self, SynthProfileError> {
        Ok(Self {
            profile: profile.validate()?,
            voices: [None; REFERENCE_MAXIMUM_VOICES],
            frame_cursor: 0,
            allocation_order: 0,
            sustain_down: false,
            pitch_bend_amount_millionths: 0,
            pitch_bend_range_microcents: 0,
            modulation_amount_millionths: 0,
            modulation_destination: ModulationDestination::Pitch,
            lfo_phase: 0,
        })
    }

    pub const fn profile(&self) -> ReferenceSynthProfile {
        self.profile
    }

    pub const fn frame_cursor(&self) -> u64 {
        self.frame_cursor
    }

    pub fn active_voice_count(&self) -> u8 {
        self.voices.iter().flatten().count() as u8
    }

    pub fn voice_for(&self, occurrence: NoteOccurrenceId) -> Option<(u8, &Voice)> {
        self.voices
            .iter()
            .enumerate()
            .find_map(|(slot, voice)| match voice {
                Some(voice) if voice.occurrence == occurrence => Some((slot as u8, voice)),
                _ => None,
            })
    }

    pub fn apply_note(
        &mut self,
        event: MusicalNoteEvent,
    ) -> Result<SynthEventOutcome, SynthEventError> {
        self.require_current_or_future(event.event_time_micros)?;
        match event.gate {
            Gate::On => self.note_on(event),
            Gate::Off => self.note_off(event),
        }
    }

    pub fn apply_control(
        &mut self,
        event: MusicalControlEvent,
    ) -> Result<SynthEventOutcome, SynthEventError> {
        self.require_current_or_future(event.event_time_micros)?;
        match event.control {
            MusicalControl::Sustain { down } => {
                let mut released = 0;
                if self.sustain_down && !down {
                    for voice in self.voices.iter_mut().flatten() {
                        if voice.key_released {
                            voice.envelope.note_off(self.profile);
                            released += 1;
                        }
                    }
                }
                self.sustain_down = down;
                Ok(SynthEventOutcome::Sustain {
                    down,
                    released_voices: released,
                })
            }
            MusicalControl::PitchBend {
                amount_millionths,
                range_microcents,
            } => {
                self.pitch_bend_amount_millionths = amount_millionths;
                self.pitch_bend_range_microcents = range_microcents;
                Ok(SynthEventOutcome::PitchBend)
            }
            MusicalControl::Modulation {
                amount_millionths,
                destination,
            } => {
                self.modulation_amount_millionths = amount_millionths;
                self.modulation_destination = destination;
                Ok(SynthEventOutcome::Modulation)
            }
        }
    }

    /// Renders exactly `output.len()` mono signed-16 frames. Callers own the
    /// finite PCM block and must reserve its Cord capacity before invoking.
    pub fn render(&mut self, output: &mut [i16]) -> RenderSummary {
        assert!(output.len() <= usize::from(self.profile.maximum_block_frames));
        let frame_count = output.len() as u16;
        let first_frame = self.frame_cursor;
        let active_before = self.active_voice_count();
        let mut clipped = 0u16;
        for sample in output {
            let mixed = self.render_sample();
            let scaled = (i64::from(mixed) * i64::from(self.profile.master_gain_q16)) >> 31;
            *sample = if scaled > i64::from(i16::MAX) {
                clipped = clipped.saturating_add(1);
                i16::MAX
            } else if scaled < i64::from(i16::MIN) {
                clipped = clipped.saturating_add(1);
                i16::MIN
            } else {
                scaled as i16
            };
            self.frame_cursor = self.frame_cursor.saturating_add(1);
            self.retire_idle_voices();
        }
        RenderSummary {
            first_frame,
            frame_count,
            active_voices_before: active_before,
            active_voices_after: self.active_voice_count(),
            clipped_samples: clipped,
        }
    }

    pub fn stop(&mut self) -> u8 {
        let released = self.active_voice_count();
        self.voices.fill(None);
        self.sustain_down = false;
        self.pitch_bend_amount_millionths = 0;
        self.pitch_bend_range_microcents = 0;
        self.modulation_amount_millionths = 0;
        self.lfo_phase = 0;
        released
    }

    fn note_on(&mut self, event: MusicalNoteEvent) -> Result<SynthEventOutcome, SynthEventError> {
        if self.voice_for(event.occurrence).is_some() {
            return Err(SynthEventError::DuplicateOccurrence);
        }
        self.allocation_order = self.allocation_order.saturating_add(1);
        let new_voice = Voice::new(
            event.occurrence,
            event.pitch,
            event.velocity,
            self.allocation_order,
            self.profile,
        );
        let admitted = usize::from(self.profile.maximum_voices);
        if let Some(slot) = self.voices[..admitted].iter().position(Option::is_none) {
            self.voices[slot] = Some(new_voice);
            return Ok(SynthEventOutcome::NoteOn(
                VoiceAllocationOutcome::Allocated { slot: slot as u8 },
            ));
        }
        if self.profile.steal_policy == VoiceStealPolicy::Refuse {
            return Err(SynthEventError::VoiceExhausted);
        }
        let slot = self.steal_slot(admitted);
        let occurrence = self.voices[slot].unwrap().occurrence;
        self.voices[slot] = Some(new_voice);
        Ok(SynthEventOutcome::NoteOn(VoiceAllocationOutcome::Stolen {
            slot: slot as u8,
            occurrence,
        }))
    }

    fn note_off(&mut self, event: MusicalNoteEvent) -> Result<SynthEventOutcome, SynthEventError> {
        let (slot, voice) = self
            .voices
            .iter_mut()
            .enumerate()
            .find_map(|(slot, voice)| {
                voice
                    .as_mut()
                    .filter(|voice| voice.occurrence == event.occurrence)
                    .map(|voice| (slot, voice))
            })
            .ok_or(SynthEventError::UnknownOccurrence)?;
        if voice.key_released {
            return Err(SynthEventError::DuplicateNoteOff);
        }
        voice.key_released = true;
        if !self.sustain_down {
            voice.envelope.note_off(self.profile);
        }
        Ok(SynthEventOutcome::NoteOff {
            slot: slot as u8,
            sustained: self.sustain_down,
        })
    }

    fn render_sample(&mut self) -> i32 {
        let lfo = triangle_lfo(self.lfo_phase);
        self.lfo_phase = self
            .lfo_phase
            .wrapping_add(lfo_increment(self.profile.lfo_rate_millihertz));
        let active = i64::from(self.active_voice_count().max(1));
        let mut mix = 0i64;
        for voice in self.voices.iter_mut().flatten() {
            let envelope = voice.envelope.next_level(self.profile);
            let increment = phase_increment(
                voice.pitch.frequency_millihertz,
                self.pitch_bend_amount_millionths,
                self.pitch_bend_range_microcents,
                lfo,
                self.modulation_amount_millionths,
                self.modulation_destination,
                self.profile.lfo_depth_q16,
            );
            let raw = voice.oscillator.sample(
                self.profile.oscillator,
                increment,
                self.profile.pulse_width_q16,
            );
            let velocity = u64::from(voice.velocity);
            let amplified = (i64::from(raw) * i64::from(envelope) * velocity as i64) >> 32;
            let cutoff = modulated_cutoff(
                self.profile,
                envelope,
                lfo,
                self.modulation_amount_millionths,
                self.modulation_destination,
            );
            let filtered = voice.filter.process(
                clamp_i64_to_i32(amplified),
                cutoff,
                self.profile.filter_resonance_q16,
            );
            mix += i64::from(filtered) / active;
        }
        clamp_i64_to_i32(mix)
    }

    fn retire_idle_voices(&mut self) {
        for voice in &mut self.voices {
            if voice.is_some_and(|voice| voice.envelope.is_idle()) {
                *voice = None;
            }
        }
    }

    fn steal_slot(&self, admitted: usize) -> usize {
        self.voices[..admitted]
            .iter()
            .enumerate()
            .min_by_key(|(_, voice)| {
                let voice = voice.unwrap();
                (!voice.key_released, voice.started_order)
            })
            .map(|(slot, _)| slot)
            .unwrap()
    }

    fn require_current_or_future(&self, micros: u64) -> Result<(), SynthEventError> {
        let frame = micros.saturating_mul(u64::from(REFERENCE_SAMPLE_RATE_HZ)) / 1_000_000;
        if frame < self.frame_cursor {
            Err(SynthEventError::StaleEventTime)
        } else {
            Ok(())
        }
    }
}

fn phase_increment(
    frequency_millihertz: u64,
    bend_amount: i32,
    bend_range_microcents: u32,
    lfo: i32,
    modulation_amount: u32,
    destination: ModulationDestination,
    lfo_depth_q16: u16,
) -> u32 {
    let base = ((u128::from(frequency_millihertz) << 32)
        / (u128::from(REFERENCE_SAMPLE_RATE_HZ) * 1_000)) as i128;
    let bend_microcents = i128::from(bend_amount) * i128::from(bend_range_microcents) / 1_000_000;
    // Frozen linearized fine-pitch scale: 1 octave (1_200_000_000
    // microcents) doubles phase increment. Exact fixtures use this profile;
    // other tuning laws require another declared implementation profile.
    let mut adjustment = bend_microcents * base / 1_200_000_000;
    if destination == ModulationDestination::Pitch {
        adjustment +=
            i128::from(lfo) * i128::from(modulation_amount) * i128::from(lfo_depth_q16) * base
                / (i128::from(i32::MAX) * 1_000_000 * 65_536);
    }
    (base + adjustment).clamp(1, i128::from(u32::MAX)) as u32
}

fn lfo_increment(rate_millihertz: u32) -> u32 {
    ((u128::from(rate_millihertz) << 32) / (u128::from(REFERENCE_SAMPLE_RATE_HZ) * 1_000)) as u32
}

fn triangle_lfo(phase: u32) -> i32 {
    let rising = phase < 0x8000_0000;
    let magnitude = ((phase & 0x7fff_ffff) >> 1) as i32;
    if rising {
        magnitude.saturating_mul(2).saturating_sub(i32::MAX)
    } else {
        i32::MAX.saturating_sub(magnitude.saturating_mul(2))
    }
}

fn modulated_cutoff(
    profile: ReferenceSynthProfile,
    envelope: u32,
    lfo: i32,
    modulation_amount: u32,
    destination: ModulationDestination,
) -> u16 {
    let envelope_delta =
        i64::from(profile.filter_envelope_amount_q16) * i64::from(envelope) / 65_536;
    let lfo_delta = if destination == ModulationDestination::FilterCutoff {
        i64::from(lfo) * i64::from(modulation_amount) * i64::from(profile.lfo_depth_q16)
            / (i64::from(i32::MAX) * 1_000_000)
    } else {
        0
    };
    (i64::from(profile.filter_cutoff_q16) + envelope_delta + lfo_delta).clamp(1, 32_768) as u16
}
