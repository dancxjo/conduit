use conduit_core::InfoDecodeError;
use conduit_form::rust_binding::NativeRustBinding;
use conduit_robotics::{WheelDropObservation, ROBOTICS_WHEEL_DROP_ENCODED_LEN, WHEEL_MASK};

#[test]
fn wheel_drop_round_trips_exact_native_and_legacy_codec_truth() {
    let value = WheelDropObservation::new(0b101).unwrap();
    assert_eq!(value.dropped_wheels(), 0b101);
    let structured = value.into_structured().unwrap();
    assert_eq!(
        WheelDropObservation::from_structured(structured).unwrap(),
        value
    );
    assert_eq!(value.encode(), [5]);
    assert_eq!(
        value.semantic_digest(),
        [
            96, 94, 119, 242, 22, 79, 64, 218, 194, 63, 211, 193, 129, 140, 75, 70, 206, 12, 138,
            28, 44, 136, 72, 127, 216, 15, 173, 160, 43, 17, 35, 186,
        ]
    );
    assert_eq!(WheelDropObservation::decode(&value.encode()), Ok(value));
}

#[test]
fn wheel_drop_enforces_mask_boundary_and_preserves_decode_refusal() {
    assert!(WheelDropObservation::new(WHEEL_MASK).is_ok());
    assert!(WheelDropObservation::new(WHEEL_MASK + 1).is_err());
    assert!(matches!(
        WheelDropObservation::decode(&[WHEEL_MASK + 1]),
        Err(InfoDecodeError::ReservedValue {
            field: "dropped-wheels",
            actual: 8,
        })
    ));
    assert!(matches!(
        WheelDropObservation::decode(&[]),
        Err(InfoDecodeError::WrongLength {
            expected: ROBOTICS_WHEEL_DROP_ENCODED_LEN,
            actual: 0,
        })
    ));
}
