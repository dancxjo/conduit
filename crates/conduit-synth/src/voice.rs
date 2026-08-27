use conduit_audio::{MusicalPitch, NoteOccurrenceId};

use crate::{Envelope, Oscillator, ReferenceSynthProfile, ResonantLowPass};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Voice {
    pub occurrence: NoteOccurrenceId,
    pub pitch: MusicalPitch,
    pub velocity: u16,
    pub started_order: u64,
    pub key_released: bool,
    pub oscillator: Oscillator,
    pub envelope: Envelope,
    pub filter: ResonantLowPass,
}

impl Voice {
    pub fn new(
        occurrence: NoteOccurrenceId,
        pitch: MusicalPitch,
        velocity: u16,
        started_order: u64,
        profile: ReferenceSynthProfile,
    ) -> Self {
        let mut envelope = Envelope::default();
        envelope.note_on(profile);
        Self {
            occurrence,
            pitch,
            velocity,
            started_order,
            key_released: false,
            oscillator: Oscillator::default(),
            envelope,
            filter: ResonantLowPass::default(),
        }
    }

    pub fn release(&mut self, profile: ReferenceSynthProfile) {
        self.key_released = true;
        self.envelope.note_off(profile);
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum VoiceAllocationOutcome {
    Allocated {
        slot: u8,
    },
    Stolen {
        slot: u8,
        occurrence: NoteOccurrenceId,
    },
    Refused,
}
