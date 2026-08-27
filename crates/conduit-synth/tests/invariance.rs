mod common;

use conduit_audio::Gate;
use conduit_synth::{ReferenceSynth, ReferenceSynthProfile, REFERENCE_MAXIMUM_VOICES};

use common::{energy, note, render_frames};

#[test]
fn equivalent_chunkings_produce_exactly_equal_pcm_and_state() {
    let profile = ReferenceSynthProfile::musician_reference();
    let mut one = ReferenceSynth::new(profile).unwrap();
    let mut odd = ReferenceSynth::new(profile).unwrap();
    for synth in [&mut one, &mut odd] {
        synth
            .apply_note(note(1, 261_626, Gate::On, 48_000, 0, 0))
            .unwrap();
        synth
            .apply_note(note(2, 329_628, Gate::On, 48_000, 0, 1))
            .unwrap();
        synth
            .apply_note(note(3, 391_995, Gate::On, 48_000, 0, 2))
            .unwrap();
    }
    let one_pcm = render_frames(&mut one, 4_096, 256);
    let odd_pcm = render_frames(&mut odd, 4_096, 73);
    assert_eq!(one_pcm, odd_pcm);
    assert_eq!(one, odd);
    assert!(energy(&one_pcm) > 0);
}

#[test]
fn held_note_has_attack_sustain_and_release_then_silence() {
    let mut synth = common::synth();
    synth
        .apply_note(note(1, 440_000, Gate::On, 50_000, 0, 0))
        .unwrap();
    let attack = render_frames(&mut synth, 480, 64);
    let sustain = render_frames(&mut synth, 6_000, 127);
    synth
        .apply_note(note(1, 440_000, Gate::Off, 0, 135_000, 1))
        .unwrap();
    let release = render_frames(&mut synth, 8_000, 89);
    assert!(energy(&attack) > 0);
    assert!(energy(&sustain) > 0);
    assert!(energy(&release) > 0);
    assert_eq!(synth.active_voice_count(), 0);
    assert!(render_frames(&mut synth, 256, 31)
        .iter()
        .all(|sample| *sample == 0));
}

#[test]
fn stop_clears_every_bounded_voice_and_dsp_state() {
    let mut synth = common::synth();
    for occurrence in 1..=8 {
        synth
            .apply_note(note(
                occurrence,
                220_000 + occurrence * 10_000,
                Gate::On,
                40_000,
                0,
                occurrence as u32,
            ))
            .unwrap();
    }
    render_frames(&mut synth, 512, 127);
    assert_eq!(synth.stop(), 8);
    assert_eq!(synth.active_voice_count(), 0);
    assert!(render_frames(&mut synth, 512, 256)
        .iter()
        .all(|sample| *sample == 0));
    assert!(core::mem::size_of::<ReferenceSynth>() < 8_192);
    assert_eq!(REFERENCE_MAXIMUM_VOICES, 16);
}
