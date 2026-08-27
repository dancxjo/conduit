#![allow(dead_code)]

use conduit_audio::{Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId};
use conduit_synth::{ReferenceSynth, ReferenceSynthProfile};

pub fn synth() -> ReferenceSynth {
    ReferenceSynth::new(ReferenceSynthProfile::musician_reference()).unwrap()
}

pub fn pitch(frequency_millihertz: u64) -> MusicalPitch {
    MusicalPitch::new(frequency_millihertz, 440_000, 0).unwrap()
}

pub fn note(
    occurrence: u64,
    frequency_millihertz: u64,
    gate: Gate,
    velocity: u16,
    micros: u64,
    order: u32,
) -> MusicalNoteEvent {
    MusicalNoteEvent::new(
        NoteOccurrenceId(occurrence),
        pitch(frequency_millihertz),
        gate,
        velocity,
        micros,
        order,
    )
    .unwrap()
}

pub fn render_frames(synth: &mut ReferenceSynth, frames: usize, chunk: usize) -> Vec<i16> {
    let mut result = Vec::with_capacity(frames);
    while result.len() < frames {
        let count = (frames - result.len()).min(chunk);
        let mut block = vec![0; count];
        synth.render(&mut block);
        result.extend(block);
    }
    result
}

pub fn energy(samples: &[i16]) -> u64 {
    samples
        .iter()
        .map(|sample| u64::from(sample.unsigned_abs()))
        .sum()
}
