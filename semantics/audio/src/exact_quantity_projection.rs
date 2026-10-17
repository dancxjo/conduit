//! Exact physical quantities project explicitly into existing bounded sound
//! records. Those records retain their millihertz/microsecond encoding and
//! range laws; this bridge does not introduce a second audio quantity family.
use conduit_core::{Quantity, Unit};

use crate::{Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId, SoundInfoError, ToneIntent};

fn time_micros(event_time: Quantity) -> Result<u64, SoundInfoError> {
    event_time
        .convert_to_u64(Unit::Microsecond)
        .map_err(SoundInfoError::QuantityConversion)
}

impl ToneIntent {
    /// Project an explicit physical offset to the existing event-time field.
    /// The caller still supplies correlation, gate and ordering authority.
    pub fn from_exact_time(
        correlation: u64,
        pitch: MusicalPitch,
        gate: Gate,
        event_time: Quantity,
        order: u32,
    ) -> Result<Self, SoundInfoError> {
        let micros = time_micros(event_time)?;
        Self::new(correlation, pitch, gate, micros, order)
            .map_err(|_| SoundInfoError::OutOfRange("tone-intent"))
    }
}

impl MusicalNoteEvent {
    /// Project physical time without changing note occurrence or gate identity.
    pub fn from_exact_time(
        occurrence: NoteOccurrenceId,
        pitch: MusicalPitch,
        gate: Gate,
        velocity: u16,
        event_time: Quantity,
        order: u32,
    ) -> Result<Self, SoundInfoError> {
        Self::new(
            occurrence,
            pitch,
            gate,
            velocity,
            time_micros(event_time)?,
            order,
        )
        .map_err(|_| SoundInfoError::OutOfRange("musical-note-event"))
    }
}
