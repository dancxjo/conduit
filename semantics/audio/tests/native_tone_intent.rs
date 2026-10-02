use conduit_audio::{Gate, MusicalPitch, ToneIntent, SOUND_TONE_INFO_ID};
use conduit_core::semantic_digest;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn tone_intent_preserves_native_identity_codec_and_digest() {
    let pitch = MusicalPitch::new(440_000, 440_000, 0).unwrap();
    let intent = ToneIntent::new(9, pitch, Gate::On, 12, 3).unwrap();
    let expected = [
        9, 0, 0, 0, 0, 0, 0, 0, 0xc0, 0xb6, 0x06, 0, 0, 0, 0, 0, 0xc0, 0xb6, 0x06, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 1, 12, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0,
    ];
    assert_eq!(intent.encode(), expected);
    assert_eq!(ToneIntent::decode(&expected), Ok(intent));
    assert_eq!(
        intent.semantic_digest(),
        semantic_digest(SOUND_TONE_INFO_ID, &expected)
    );

    let structured = intent.into_structured().unwrap();
    assert_eq!(ToneIntent::from_structured(structured).unwrap(), intent);
}

#[test]
fn tone_intent_native_constructor_owns_exact_bounds() {
    let pitch = MusicalPitch::new(440_000, 440_000, 0).unwrap();
    assert!(ToneIntent::new(0, pitch, Gate::On, 0, 0).is_err());
    assert!(ToneIntent::new(1, pitch, Gate::On, u64::MAX, 0).is_err());
    assert!(ToneIntent::new(1, pitch, Gate::Off, u64::MAX - 1, u32::MAX).is_ok());
}
