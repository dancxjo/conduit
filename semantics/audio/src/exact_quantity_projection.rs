//! Exact physical quantities project explicitly into existing bounded sound
//! records. Those records retain their millihertz/microsecond encoding and
//! range laws; this bridge does not introduce a second audio quantity family.
use conduit_core::{ExactDecimalQuantity, QuantityUnit};

use crate::{Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId, SoundInfoError, ToneIntent};

fn time_micros(event_time: ExactDecimalQuantity) -> Result<u64, SoundInfoError> {
    event_time
        .convert_to_u64(QuantityUnit::Microsecond)
        .map_err(SoundInfoError::QuantityConversion)
}

impl MusicalPitch {
    /// Admit physical frequency and tuning through the common exact law.
    /// Sub-millihertz precision refuses rather than rounding a pitch.
    pub fn from_exact_quantities(
        frequency: ExactDecimalQuantity,
        a4_reference: ExactDecimalQuantity,
        detune_microcents: i32,
    ) -> Result<Self, SoundInfoError> {
        let frequency = frequency
            .convert_to_u64(QuantityUnit::Millihertz)
            .map_err(SoundInfoError::QuantityConversion)?;
        let reference = a4_reference
            .convert_to_u64(QuantityUnit::Millihertz)
            .map_err(SoundInfoError::QuantityConversion)?;
        Self::new(frequency, reference, detune_microcents)
            .map_err(|_| SoundInfoError::OutOfRange("musical-pitch"))
    }
}

impl ToneIntent {
    /// Project an explicit physical offset to the existing event-time field.
    /// The caller still supplies correlation, gate and ordering authority.
    pub fn from_exact_time(
        correlation: u64,
        pitch: MusicalPitch,
        gate: Gate,
        event_time: ExactDecimalQuantity,
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
        event_time: ExactDecimalQuantity,
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
