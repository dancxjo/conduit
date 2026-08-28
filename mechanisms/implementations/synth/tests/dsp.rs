mod common;

use conduit_audio::{Gate, ModulationDestination, MusicalControl, MusicalControlEvent};
use conduit_synth::{OscillatorShape, ReferenceSynth, ReferenceSynthProfile};

use common::{energy, note, render_frames};

fn rendered(profile: ReferenceSynthProfile, velocity: u16) -> Vec<i16> {
    let mut synth = ReferenceSynth::new(profile).unwrap();
    synth
        .apply_note(note(1, 440_000, Gate::On, velocity, 0, 0))
        .unwrap();
    render_frames(&mut synth, 2_048, 256)
}

#[test]
fn four_oscillators_have_distinct_deterministic_output() {
    let mut profile = ReferenceSynthProfile::musician_reference();
    let mut outputs = Vec::new();
    for shape in [
        OscillatorShape::Sine,
        OscillatorShape::Triangle,
        OscillatorShape::Saw,
        OscillatorShape::Pulse,
    ] {
        profile.oscillator = shape;
        outputs.push(rendered(profile, u16::MAX));
    }
    for left in 0..outputs.len() {
        for right in left + 1..outputs.len() {
            assert_ne!(outputs[left], outputs[right]);
        }
    }
    assert_eq!(&outputs[2][..4], &[0, -2, -5, -12]);
}

#[test]
fn velocity_changes_objective_signal_energy() {
    let profile = ReferenceSynthProfile::musician_reference();
    let quiet = rendered(profile, 8_000);
    let loud = rendered(profile, 56_000);
    assert!(energy(&loud) > energy(&quiet) * 4);
}

#[test]
fn filter_resonance_cutoff_and_envelope_amount_change_output() {
    let base = ReferenceSynthProfile::musician_reference();
    let baseline = rendered(base, 50_000);
    let mut changed = base;
    changed.filter_cutoff_q16 = 8_000;
    changed.filter_resonance_q16 = 48_000;
    changed.filter_envelope_amount_q16 = -6_000;
    let filtered = rendered(changed, 50_000);
    assert_ne!(baseline, filtered);
    assert_ne!(energy(&baseline), energy(&filtered));
}

#[test]
fn pitch_bend_and_lfo_modulation_change_normalized_output() {
    let profile = ReferenceSynthProfile::musician_reference();
    let plain = rendered(profile, 50_000);

    let mut bent = ReferenceSynth::new(profile).unwrap();
    bent.apply_note(note(1, 440_000, Gate::On, 50_000, 0, 0))
        .unwrap();
    bent.apply_control(
        MusicalControlEvent::new(
            MusicalControl::PitchBend {
                amount_millionths: 500_000,
                range_microcents: 200_000_000,
            },
            0,
            1,
        )
        .unwrap(),
    )
    .unwrap();
    let bent = render_frames(&mut bent, 2_048, 256);
    assert_ne!(plain, bent);

    let mut modulated = ReferenceSynth::new(profile).unwrap();
    modulated
        .apply_note(note(1, 440_000, Gate::On, 50_000, 0, 0))
        .unwrap();
    modulated
        .apply_control(
            MusicalControlEvent::new(
                MusicalControl::Modulation {
                    amount_millionths: 1_000_000,
                    destination: ModulationDestination::FilterCutoff,
                },
                0,
                1,
            )
            .unwrap(),
        )
        .unwrap();
    let modulated = render_frames(&mut modulated, 2_048, 256);
    assert_ne!(plain, modulated);
}
