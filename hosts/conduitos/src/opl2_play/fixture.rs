//! Portable reviewed note fixture reused by OPL conformance and later S8 seams.

use conduit_core::{Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId};

use super::EVENTS;

pub fn reviewed_values() -> [MusicalNoteEvent; EVENTS] {
    let mut values = [note(1, 440_000, Gate::On, 0); EVENTS];
    let sequence = [
        (2, 261_626, Gate::On),
        (3, 329_628, Gate::On),
        (4, 391_995, Gate::On),
        (2, 261_626, Gate::Off),
        (3, 329_628, Gate::Off),
        (4, 391_995, Gate::Off),
        (10, 220_000, Gate::On),
        (11, 246_942, Gate::On),
        (12, 277_183, Gate::On),
        (13, 293_665, Gate::On),
        (14, 329_628, Gate::On),
        (15, 369_994, Gate::On),
        (16, 415_305, Gate::On),
        (17, 466_164, Gate::On),
        (18, 523_251, Gate::On),
        (10, 220_000, Gate::Off),
        (11, 246_942, Gate::Off),
        (12, 277_183, Gate::Off),
        (13, 293_665, Gate::Off),
        (14, 329_628, Gate::Off),
        (15, 369_994, Gate::Off),
        (16, 415_305, Gate::Off),
        (17, 466_164, Gate::Off),
        (18, 523_251, Gate::Off),
    ];
    for (index, (occurrence, frequency, gate)) in sequence.into_iter().enumerate() {
        values[index] = note(occurrence, frequency, gate, index as u32);
    }
    values
}

pub(super) fn note(occurrence: u64, frequency: u64, gate: Gate, order: u32) -> MusicalNoteEvent {
    MusicalNoteEvent::new(
        NoteOccurrenceId(occurrence),
        MusicalPitch::new(frequency, 440_000, 0).expect("reviewed pitch"),
        gate,
        u16::MAX,
        u64::from(order) * 1_000,
        order,
    )
    .expect("reviewed note")
}
