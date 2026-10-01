use conduit_core::InfoDecodeError;
use conduit_form::rust_binding::NativeRustBinding;
use conduit_robotics::{
    OdometryObservation, MAXIMUM_ODOMETRY_MM, PI_MICRORADIANS, ROBOTICS_ODOMETRY_ENCODED_LEN,
};

#[test]
fn odometry_round_trips_exact_native_and_legacy_codec_truth() {
    let value = OdometryObservation::new(1_234, -5_678, PI_MICRORADIANS).unwrap();
    assert_eq!(value.components(), (1_234, -5_678, PI_MICRORADIANS));
    let structured = value.into_structured().unwrap();
    assert_eq!(
        OdometryObservation::from_structured(structured).unwrap(),
        value
    );
    assert_eq!(
        value.encode(),
        [210, 4, 0, 0, 210, 233, 255, 255, 217, 239, 47, 0]
    );
    assert_eq!(
        value.semantic_digest(),
        [
            103, 103, 139, 104, 216, 110, 114, 60, 248, 160, 117, 103, 110, 216, 147, 142, 232,
            222, 33, 217, 143, 148, 21, 187, 253, 225, 184, 117, 63, 230, 170, 188,
        ]
    );
    assert_eq!(OdometryObservation::decode(&value.encode()), Ok(value));
}

#[test]
fn odometry_enforces_every_boundary_and_preserves_decode_refusal() {
    assert!(
        OdometryObservation::new(-MAXIMUM_ODOMETRY_MM, MAXIMUM_ODOMETRY_MM, -PI_MICRORADIANS,)
            .is_ok()
    );
    assert!(OdometryObservation::new(MAXIMUM_ODOMETRY_MM + 1, 0, 0).is_err());
    assert!(OdometryObservation::new(0, -MAXIMUM_ODOMETRY_MM - 1, 0).is_err());
    assert!(OdometryObservation::new(0, 0, PI_MICRORADIANS + 1).is_err());

    let mut encoded = [0_u8; ROBOTICS_ODOMETRY_ENCODED_LEN];
    encoded[..4].copy_from_slice(&(MAXIMUM_ODOMETRY_MM + 1).to_le_bytes());
    assert!(matches!(
        OdometryObservation::decode(&encoded),
        Err(InfoDecodeError::OutOfRange {
            field: "forward-mm",
            minimum: -10_000_000,
            maximum: 10_000_000,
            actual: 10_000_001,
        })
    ));
    assert!(matches!(
        OdometryObservation::decode(&encoded[..11]),
        Err(InfoDecodeError::WrongLength {
            expected: ROBOTICS_ODOMETRY_ENCODED_LEN,
            actual: 11,
        })
    ));
}
