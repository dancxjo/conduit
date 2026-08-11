mod common;

use conduit_core::{Gate, MusicalControl, MusicalControlEvent, NoteOccurrenceId};
use conduit_synth::{
    EnvelopeStage, SynthEventError, SynthEventOutcome, VoiceAllocationOutcome, VoiceStealPolicy,
};

use common::{note, render_frames, synth};

#[test]
fn overlapping_equal_pitch_occurrences_release_independently() {
    let mut synth = synth();
    synth
        .apply_note(note(1, 261_626, Gate::On, 50_000, 0, 0))
        .unwrap();
    synth
        .apply_note(note(2, 261_626, Gate::On, 50_000, 0, 1))
        .unwrap();
    assert_eq!(synth.active_voice_count(), 2);
    synth
        .apply_note(note(1, 261_626, Gate::Off, 0, 0, 2))
        .unwrap();
    assert!(synth.voice_for(NoteOccurrenceId(2)).is_some());
    assert!(!synth.voice_for(NoteOccurrenceId(2)).unwrap().1.key_released);
    render_frames(&mut synth, 8_000, 127);
    assert!(synth.voice_for(NoteOccurrenceId(1)).is_none());
    assert!(synth.voice_for(NoteOccurrenceId(2)).is_some());
}

#[test]
fn sustain_holds_key_release_then_releases_exact_voices() {
    let mut synth = synth();
    synth
        .apply_note(note(3, 440_000, Gate::On, 40_000, 0, 0))
        .unwrap();
    synth
        .apply_control(
            MusicalControlEvent::new(MusicalControl::Sustain { down: true }, 0, 1).unwrap(),
        )
        .unwrap();
    let outcome = synth
        .apply_note(note(3, 440_000, Gate::Off, 0, 0, 2))
        .unwrap();
    assert_eq!(
        outcome,
        SynthEventOutcome::NoteOff {
            slot: 0,
            sustained: true
        }
    );
    render_frames(&mut synth, 10_000, 256);
    assert_eq!(
        synth
            .voice_for(NoteOccurrenceId(3))
            .unwrap()
            .1
            .envelope
            .stage(),
        EnvelopeStage::Sustain
    );
    assert_eq!(
        synth
            .apply_control(
                MusicalControlEvent::new(MusicalControl::Sustain { down: false }, 208_334, 3,)
                    .unwrap()
            )
            .unwrap(),
        SynthEventOutcome::Sustain {
            down: false,
            released_voices: 1
        }
    );
    render_frames(&mut synth, 8_000, 256);
    assert!(synth.voice_for(NoteOccurrenceId(3)).is_none());
}

#[test]
fn exhaustion_steals_oldest_released_then_oldest_active() {
    let mut synth = synth();
    for occurrence in 1..=8 {
        synth
            .apply_note(note(
                occurrence,
                220_000 + occurrence * 1_000,
                Gate::On,
                32_000,
                0,
                occurrence as u32,
            ))
            .unwrap();
    }
    synth
        .apply_note(note(3, 223_000, Gate::Off, 0, 0, 9))
        .unwrap();
    assert_eq!(
        synth
            .apply_note(note(9, 330_000, Gate::On, 32_000, 0, 10))
            .unwrap(),
        SynthEventOutcome::NoteOn(VoiceAllocationOutcome::Stolen {
            slot: 2,
            occurrence: NoteOccurrenceId(3)
        })
    );
    assert!(synth.voice_for(NoteOccurrenceId(3)).is_none());

    let mut refusing_profile = synth.profile();
    refusing_profile.steal_policy = VoiceStealPolicy::Refuse;
    let mut refusing = conduit_synth::ReferenceSynth::new(refusing_profile).unwrap();
    for occurrence in 1..=8 {
        refusing
            .apply_note(note(
                occurrence,
                440_000,
                Gate::On,
                32_000,
                0,
                occurrence as u32,
            ))
            .unwrap();
    }
    assert_eq!(
        refusing.apply_note(note(9, 440_000, Gate::On, 32_000, 0, 9)),
        Err(SynthEventError::VoiceExhausted)
    );
}

#[test]
fn duplicate_and_stale_note_off_never_kill_another_occurrence() {
    let mut synth = synth();
    synth
        .apply_note(note(1, 440_000, Gate::On, 32_000, 0, 0))
        .unwrap();
    synth
        .apply_note(note(2, 440_000, Gate::On, 32_000, 0, 1))
        .unwrap();
    synth
        .apply_note(note(1, 440_000, Gate::Off, 0, 0, 2))
        .unwrap();
    assert_eq!(
        synth.apply_note(note(1, 440_000, Gate::Off, 0, 0, 3)),
        Err(SynthEventError::DuplicateNoteOff)
    );
    assert_eq!(
        synth.apply_note(note(99, 440_000, Gate::Off, 0, 0, 4)),
        Err(SynthEventError::UnknownOccurrence)
    );
    assert!(!synth.voice_for(NoteOccurrenceId(2)).unwrap().1.key_released);
}
