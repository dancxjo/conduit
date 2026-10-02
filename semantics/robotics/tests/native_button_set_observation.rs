use conduit_core::InfoDecodeError;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_robotics::{ButtonSetObservation, ROBOTICS_BUTTONS_ENCODED_LEN};

#[test]
fn button_set_round_trips_exact_native_and_legacy_codec_truth() {
    let value = ButtonSetObservation::new(0x8000_0001).unwrap();
    assert_eq!(value.pressed(), 0x8000_0001);
    let structured = value.into_structured().unwrap();
    assert_eq!(
        ButtonSetObservation::from_structured(structured).unwrap(),
        value
    );
    assert_eq!(value.encode(), [1, 0, 0, 128]);
    assert_eq!(
        value.semantic_digest(),
        [
            140, 80, 246, 227, 121, 136, 206, 157, 136, 143, 46, 179, 69, 229, 87, 69, 88, 41, 194,
            13, 229, 183, 44, 80, 69, 53, 98, 248, 45, 17, 186, 176,
        ]
    );
    assert_eq!(ButtonSetObservation::decode(&value.encode()), Ok(value));
}

#[test]
fn button_set_covers_full_u32_domain_and_preserves_decode_refusal() {
    assert!(ButtonSetObservation::new(u32::MIN).is_ok());
    assert!(ButtonSetObservation::new(u32::MAX).is_ok());
    assert!(matches!(
        ButtonSetObservation::decode(&[0; 3]),
        Err(InfoDecodeError::WrongLength {
            expected: ROBOTICS_BUTTONS_ENCODED_LEN,
            actual: 3,
        })
    ));
}
