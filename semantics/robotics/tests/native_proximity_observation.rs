use conduit_core::InfoDecodeError;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_robotics::{
    ProximityObservation, BODY_SECTOR_FRONT_LEFT, BODY_SECTOR_MASK, ROBOTICS_PROXIMITY_ENCODED_LEN,
};

#[test]
fn proximity_round_trips_and_preserves_codec_and_digest() {
    let value = ProximityObservation::new(BODY_SECTOR_FRONT_LEFT).unwrap();
    let structured = value.into_structured().unwrap();
    assert_eq!(
        ProximityObservation::from_structured(structured).unwrap(),
        value
    );
    assert_eq!(ProximityObservation::encode(value), [2]);
    assert_eq!(
        value.semantic_digest(),
        [
            53, 165, 58, 225, 48, 13, 215, 207, 231, 84, 173, 44, 87, 202, 84, 30, 57, 9, 125, 233,
            111, 98, 125, 142, 50, 197, 10, 197, 168, 47, 207, 114,
        ]
    );
    assert_eq!(ProximityObservation::decode(&[2]), Ok(value));
}

#[test]
fn proximity_enforces_sector_mask_and_preserves_typed_decode_refusals() {
    assert!(ProximityObservation::new(0).is_ok());
    assert!(ProximityObservation::new(BODY_SECTOR_MASK).is_ok());
    assert!(ProximityObservation::new(BODY_SECTOR_MASK + 1).is_err());
    assert!(matches!(
        ProximityObservation::decode(&[0x80]),
        Err(InfoDecodeError::ReservedValue {
            field: "proximity-body-sectors",
            actual: 0x80,
        })
    ));
    assert!(matches!(
        ProximityObservation::decode(&[0, 0]),
        Err(InfoDecodeError::WrongLength {
            expected: ROBOTICS_PROXIMITY_ENCODED_LEN,
            actual: 2,
        })
    ));
}
