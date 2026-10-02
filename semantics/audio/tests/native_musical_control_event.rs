use conduit_audio::{
    MusicalControl, MusicalControlEvent, SoundInfoError, CONTROL_EVENT_ENCODED_LEN,
    MAXIMUM_EVENT_TIME_MICROS,
};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn musical_control_event_round_trips_and_preserves_codec_and_digest() {
    let value = MusicalControlEvent::new(MusicalControl::sustain(true).unwrap(), 14, 5).unwrap();
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        MusicalControlEvent::from_structured(structured).unwrap(),
        value
    );
    assert_eq!(
        MusicalControlEvent::encode(&value),
        [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 14, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0]
    );
    assert_eq!(
        value.semantic_digest(),
        [
            169, 224, 240, 54, 212, 210, 88, 241, 48, 139, 154, 240, 101, 35, 226, 232, 159, 236,
            252, 26, 172, 138, 87, 76, 14, 50, 34, 37, 138, 140, 140, 106,
        ]
    );
    assert_eq!(
        MusicalControlEvent::decode(&MusicalControlEvent::encode(&value)),
        Ok(value)
    );
}

#[test]
fn musical_control_event_enforces_time_boundary_and_typed_decode_refusals() {
    let control = || MusicalControl::sustain(false).unwrap();
    assert!(MusicalControlEvent::new(control(), 0, 0).is_ok());
    assert!(MusicalControlEvent::new(control(), MAXIMUM_EVENT_TIME_MICROS, u32::MAX).is_ok());
    assert!(MusicalControlEvent::new(control(), u64::MAX, 0).is_err());

    let mut encoded = [0_u8; CONTROL_EVENT_ENCODED_LEN];
    encoded[10..18].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        MusicalControlEvent::decode(&encoded),
        Err(SoundInfoError::OutOfRange("event-time-micros"))
    );
    assert!(matches!(
        MusicalControlEvent::decode(&encoded[..21]),
        Err(SoundInfoError::WrongLength {
            expected: CONTROL_EVENT_ENCODED_LEN,
            actual: 21,
        })
    ));
}
