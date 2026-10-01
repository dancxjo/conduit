use conduit_core::InfoDecodeError;
use conduit_robotics::{
    OrientationObservation, HALF_PI_MICRORADIANS, PI_MICRORADIANS,
    ROBOTICS_ORIENTATION_ENCODED_LEN, ROBOTICS_ORIENTATION_INFO_ID,
};

#[test]
fn orientation_preserves_exact_native_bounds_wire_and_digest() {
    let value =
        OrientationObservation::new(-PI_MICRORADIANS, HALF_PI_MICRORADIANS, PI_MICRORADIANS)
            .unwrap();
    assert_eq!(
        value.components(),
        (-PI_MICRORADIANS, HALF_PI_MICRORADIANS, PI_MICRORADIANS)
    );
    assert_eq!(
        value.encode(),
        [0x27, 0x10, 0xd0, 0xff, 0xed, 0xf7, 0x17, 0x00, 0xd9, 0xef, 0x2f, 0x00]
    );
    assert_eq!(OrientationObservation::decode(&value.encode()), Ok(value));
    assert_eq!(ROBOTICS_ORIENTATION_ENCODED_LEN, 12);
    assert_eq!(
        ROBOTICS_ORIENTATION_INFO_ID,
        "robotics/orientation-microrad-body@1"
    );
    assert_eq!(
        value.semantic_digest(),
        [
            231, 69, 43, 195, 49, 111, 186, 227, 253, 43, 242, 126, 153, 2, 74, 140, 90, 59, 134,
            123, 130, 105, 221, 99, 233, 0, 175, 189, 248, 129, 149, 246,
        ]
    );
}

#[test]
fn orientation_refuses_each_invalid_axis_and_length() {
    assert!(OrientationObservation::new(-PI_MICRORADIANS, -HALF_PI_MICRORADIANS, 0).is_ok());
    assert!(OrientationObservation::new(PI_MICRORADIANS + 1, 0, 0).is_err());
    assert!(OrientationObservation::new(0, HALF_PI_MICRORADIANS + 1, 0).is_err());
    assert!(OrientationObservation::new(0, 0, -PI_MICRORADIANS - 1).is_err());
    assert_eq!(
        OrientationObservation::decode(&[0; 11]),
        Err(InfoDecodeError::WrongLength {
            expected: 12,
            actual: 11,
        })
    );
}
