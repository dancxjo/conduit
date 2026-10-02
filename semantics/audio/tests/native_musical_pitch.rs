use conduit_audio::MusicalPitch;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn musical_pitch_has_exact_native_and_twenty_byte_round_trips() {
    let pitch = MusicalPitch::new(440_127, 442_000, 12_500).unwrap();
    let structured = pitch.into_structured().unwrap();
    assert_eq!(
        structured.value_type(),
        &MusicalPitch::semantic_type().unwrap()
    );
    assert_eq!(MusicalPitch::from_structured(structured).unwrap(), pitch);

    let encoded = pitch.encode();
    assert_eq!(
        encoded,
        [0x3f, 0xb7, 0x06, 0, 0, 0, 0, 0, 0x90, 0xbe, 0x06, 0, 0, 0, 0, 0, 0xd4, 0x30, 0, 0,]
    );
    assert_eq!(MusicalPitch::decode(&encoded).unwrap(), pitch);
}

#[test]
fn musical_pitch_bounds_belong_to_the_native_constructor() {
    assert!(MusicalPitch::new(7_999, 440_000, 0).is_err());
    assert!(MusicalPitch::new(40_000_001, 440_000, 0).is_err());
    assert!(MusicalPitch::new(440_000, 399_999, 0).is_err());
    assert!(MusicalPitch::new(440_000, 480_001, 0).is_err());
    assert!(MusicalPitch::new(440_000, 440_000, -24_000_001).is_err());
    assert!(MusicalPitch::new(440_000, 440_000, 24_000_001).is_err());
}
