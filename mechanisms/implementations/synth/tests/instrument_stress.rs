mod common;

use conduit_audio::{
    Gate, ModulationDestination, MusicalControl, MusicalControlEvent, NoteOccurrenceId,
};
use conduit_synth::{SynthEventOutcome, VoiceAllocationOutcome};

use common::{note, synth};

const BLOCK_FRAMES: usize = 256;
const STRESS_ROUNDS: u64 = 512;

fn current_micros(frame_cursor: u64) -> u64 {
    frame_cursor.saturating_mul(1_000_000).div_ceil(48_000)
}

#[test]
fn bounded_musician_stress_ends_in_immediate_silence() {
    let mut synth = synth();
    let mut block = [0i16; BLOCK_FRAMES];
    let mut order = 0u32;
    let mut stolen = 0u32;

    for round in 0..STRESS_ROUNDS {
        let now = current_micros(synth.frame_cursor());
        let occurrence = round.saturating_mul(9).saturating_add(1);

        for chord_voice in 0..9u64 {
            order = order.saturating_add(1);
            let outcome = synth
                .apply_note(note(
                    occurrence + chord_voice,
                    220_000 + chord_voice.saturating_mul(27_500),
                    Gate::On,
                    8_000 + ((round + chord_voice) % 48_000) as u16,
                    now,
                    order,
                ))
                .unwrap();
            if matches!(
                outcome,
                SynthEventOutcome::NoteOn(VoiceAllocationOutcome::Stolen { .. })
            ) {
                stolen = stolen.saturating_add(1);
            }
        }

        order = order.saturating_add(1);
        synth
            .apply_control(
                MusicalControlEvent::new(
                    MusicalControl::PitchBend {
                        amount_millionths: if round % 2 == 0 { 500_000 } else { -500_000 },
                        range_microcents: 200_000_000,
                    },
                    now,
                    order,
                )
                .unwrap(),
            )
            .unwrap();
        order = order.saturating_add(1);
        synth
            .apply_control(
                MusicalControlEvent::new(
                    MusicalControl::Modulation {
                        amount_millionths: ((round % 11) * 100_000) as u32,
                        destination: ModulationDestination::FilterCutoff,
                    },
                    now,
                    order,
                )
                .unwrap(),
            )
            .unwrap();
        order = order.saturating_add(1);
        synth
            .apply_control(
                MusicalControlEvent::new(
                    MusicalControl::Sustain {
                        down: round % 2 == 0,
                    },
                    now,
                    order,
                )
                .unwrap(),
            )
            .unwrap();
        synth.render(&mut block);
        assert!(block.iter().any(|sample| *sample != 0));
    }

    assert_eq!(stolen, 1 + (STRESS_ROUNDS as u32 - 1) * 9);
    assert_eq!(synth.active_voice_count(), 8);
    assert_eq!(synth.stop(), 8);
    assert_eq!(synth.active_voice_count(), 0);
    assert!(synth.voice_for(NoteOccurrenceId(1)).is_none());

    block.fill(i16::MAX);
    let summary = synth.render(&mut block);
    assert_eq!(summary.active_voices_before, 0);
    assert_eq!(summary.active_voices_after, 0);
    assert!(block.iter().all(|sample| *sample == 0));
}
