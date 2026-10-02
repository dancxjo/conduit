use conduit_core::InfoDecodeError;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_robotics::{
    AccelerationObservation, MAXIMUM_ACCELERATION_MM_S2, ROBOTICS_ACCELERATION_ENCODED_LEN,
};

#[test]
fn acceleration_round_trips_exact_native_and_legacy_codec_truth() {
    let value = AccelerationObservation::new(9_810, -20, 0).unwrap();
    let structured = value.into_structured().unwrap();
    assert_eq!(
        AccelerationObservation::from_structured(structured).unwrap(),
        value
    );
    assert_eq!(
        value.encode(),
        [82, 38, 0, 0, 236, 255, 255, 255, 0, 0, 0, 0]
    );
    assert_eq!(
        value.semantic_digest(),
        [
            182, 132, 174, 196, 95, 46, 21, 60, 44, 194, 212, 192, 214, 245, 39, 232, 170, 27, 132,
            252, 120, 248, 121, 59, 193, 27, 236, 178, 129, 2, 85, 174,
        ]
    );
    assert_eq!(AccelerationObservation::decode(&value.encode()), Ok(value));
}

#[test]
fn acceleration_enforces_every_axis_boundary_and_preserves_decode_refusal() {
    let maximum = MAXIMUM_ACCELERATION_MM_S2;
    assert!(AccelerationObservation::new(-maximum, maximum, 0).is_ok());
    assert!(AccelerationObservation::new(-maximum - 1, 0, 0).is_err());
    assert!(AccelerationObservation::new(0, maximum + 1, 0).is_err());
    assert!(AccelerationObservation::new(0, 0, -maximum - 1).is_err());

    let mut encoded = [0_u8; ROBOTICS_ACCELERATION_ENCODED_LEN];
    encoded[..4].copy_from_slice(&(maximum + 1).to_le_bytes());
    assert!(matches!(
        AccelerationObservation::decode(&encoded),
        Err(InfoDecodeError::OutOfRange {
            field: "x-forward-mm-s2",
            minimum: -200_000,
            maximum: 200_000,
            actual: 200_001,
        })
    ));
    assert!(matches!(
        AccelerationObservation::decode(&encoded[..11]),
        Err(InfoDecodeError::WrongLength {
            expected: ROBOTICS_ACCELERATION_ENCODED_LEN,
            actual: 11,
        })
    ));
}
