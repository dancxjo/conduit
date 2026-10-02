use conduit_audio::{Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId, MUSIC_NOTE_INFO_ID};
use conduit_core::semantic_digest;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn note_occurrence_and_event_have_exact_native_round_trips() {
    let occurrence = NoteOccurrenceId::new(9).unwrap();
    let structured = occurrence.into_structured().unwrap();
    assert_eq!(
        NoteOccurrenceId::from_structured(structured).unwrap(),
        occurrence
    );
    assert!(NoteOccurrenceId::new(0).is_err());
    assert!(NoteOccurrenceId::new(u64::MAX).is_ok());

    let pitch = MusicalPitch::new(440_000, 440_000, 0).unwrap();
    let event = MusicalNoteEvent::new(occurrence, pitch, Gate::On, 0x1234, 12, 3).unwrap();
    let structured = event.into_structured().unwrap();
    assert_eq!(
        MusicalNoteEvent::from_structured(structured).unwrap(),
        event
    );
}

#[test]
fn musical_note_event_preserves_exact_codec_digest_and_bounds() {
    let occurrence = NoteOccurrenceId::new(9).unwrap();
    let pitch = MusicalPitch::new(440_000, 440_000, 0).unwrap();
    let event = MusicalNoteEvent::new(occurrence, pitch, Gate::On, 0x1234, 12, 3).unwrap();
    let expected = [
        9, 0, 0, 0, 0, 0, 0, 0, 0xc0, 0xb6, 0x06, 0, 0, 0, 0, 0, 0xc0, 0xb6, 0x06, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 1, 0x34, 0x12, 12, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0,
    ];
    assert_eq!(event.encode(), expected);
    assert_eq!(MusicalNoteEvent::decode(&expected), Ok(event));
    assert_eq!(
        event.semantic_digest(),
        semantic_digest(MUSIC_NOTE_INFO_ID, &expected)
    );
    assert!(MusicalNoteEvent::new(occurrence, pitch, Gate::On, 0, u64::MAX, 0).is_err());
    assert!(MusicalNoteEvent::new(
        occurrence,
        pitch,
        Gate::Off,
        u16::MAX,
        u64::MAX - 1,
        u32::MAX
    )
    .is_ok());
}
