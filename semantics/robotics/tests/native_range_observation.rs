use conduit_core::InfoDecodeError;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_robotics::{
    RangeObservation, MAXIMUM_OBSERVATION_AGE_MS, MAXIMUM_RANGE_MM, ROBOTICS_RANGE_ENCODED_LEN,
};

#[test]
fn range_round_trips_exact_native_and_legacy_codec_truth() {
    let value = RangeObservation::new(420, 12).unwrap();
    assert_eq!(value.distance_mm(), 420);
    assert_eq!(value.age_ms(), 12);
    let structured = value.into_structured().unwrap();
    assert_eq!(
        RangeObservation::from_structured(structured).unwrap(),
        value
    );
    assert_eq!(value.encode(), [164, 1, 0, 0, 12, 0, 0, 0]);
    assert_eq!(
        value.semantic_digest(),
        [
            234, 95, 113, 88, 217, 167, 200, 27, 117, 40, 79, 142, 223, 20, 190, 62, 213, 163, 66,
            237, 143, 122, 109, 179, 61, 241, 3, 160, 38, 44, 70, 230,
        ]
    );
    assert_eq!(RangeObservation::decode(&value.encode()), Ok(value));
}

#[test]
fn range_enforces_both_boundaries_and_preserves_decode_refusal() {
    assert!(RangeObservation::new(MAXIMUM_RANGE_MM, MAXIMUM_OBSERVATION_AGE_MS).is_ok());
    assert!(RangeObservation::new(MAXIMUM_RANGE_MM + 1, 0).is_err());
    assert!(RangeObservation::new(0, MAXIMUM_OBSERVATION_AGE_MS + 1).is_err());

    let mut encoded = [0_u8; ROBOTICS_RANGE_ENCODED_LEN];
    encoded[..4].copy_from_slice(&(MAXIMUM_RANGE_MM + 1).to_le_bytes());
    assert!(matches!(
        RangeObservation::decode(&encoded),
        Err(InfoDecodeError::OutOfRange {
            field: "distance-mm",
            minimum: 0,
            maximum: 1_000_000,
            actual: 1_000_001,
        })
    ));
    assert!(matches!(
        RangeObservation::decode(&encoded[..7]),
        Err(InfoDecodeError::WrongLength {
            expected: ROBOTICS_RANGE_ENCODED_LEN,
            actual: 7,
        })
    ));
}
