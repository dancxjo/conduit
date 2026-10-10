mod common;

use conduit_audio::{Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId};
use conduit_core::Quantity;

fn quantity(source: &str) -> Quantity {
    Quantity::parse_plot_literal(source).unwrap()
}

#[test]
fn exact_prefixed_pitch_and_time_feed_existing_bounded_synth_unchanged() {
    let expected_pitch = MusicalPitch::new(440_127, 440_000, 0).unwrap();
    let projected_pitch =
        MusicalPitch::from_exact_quantities(quantity("0.440127kHz"), quantity("440000000µHz"), 0)
            .unwrap();
    let mut expected = common::synth();
    let mut projected = common::synth();
    let occurrence = NoteOccurrenceId::new(9).unwrap();
    for (gate, velocity, source_time, expected_time, frames, order) in [
        (Gate::On, 50_000, "0qs", 0, 4096, 0),
        (Gate::Off, 0, "85.334ms", 85_334, 8000, 1),
    ] {
        let reference = MusicalNoteEvent::new(
            occurrence,
            expected_pitch,
            gate,
            velocity,
            expected_time,
            order,
        )
        .unwrap();
        let event = MusicalNoteEvent::from_exact_time(
            occurrence,
            projected_pitch,
            gate,
            velocity,
            quantity(source_time),
            order,
        )
        .unwrap();
        assert_eq!(event.encode(), reference.encode());
        assert_eq!(
            projected.apply_note(event).unwrap(),
            expected.apply_note(reference).unwrap()
        );
        let expected_pcm = common::render_frames(&mut expected, frames, 256);
        let actual_pcm = common::render_frames(&mut projected, frames, 73);
        assert_eq!(actual_pcm, expected_pcm);
        assert!(common::energy(&actual_pcm) > 0);
        assert_eq!(projected, expected);
    }
    assert_eq!(projected.active_voice_count(), 0);
}
