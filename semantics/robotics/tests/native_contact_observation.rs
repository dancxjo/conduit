use conduit_core::InfoDecodeError;
use conduit_form::rust_binding::NativeRustBinding;
use conduit_robotics::{ContactObservation, BODY_SECTOR_MASK, ROBOTICS_CONTACT_ENCODED_LEN};

#[test]
fn contact_round_trips_exact_native_and_legacy_codec_truth() {
    let value = ContactObservation::new(0b1_0011).unwrap();
    assert_eq!(value.active_body_sectors(), 0b1_0011);
    let structured = value.into_structured().unwrap();
    assert_eq!(
        ContactObservation::from_structured(structured).unwrap(),
        value
    );
    assert_eq!(value.encode(), [0b1_0011]);
    assert_eq!(
        value.semantic_digest(),
        [
            249, 2, 189, 90, 148, 200, 167, 13, 210, 227, 59, 7, 224, 129, 159, 147, 40, 30, 168,
            71, 86, 15, 113, 183, 107, 97, 211, 169, 99, 37, 34, 225,
        ]
    );
    assert_eq!(ContactObservation::decode(&value.encode()), Ok(value));
}

#[test]
fn contact_enforces_mask_boundary_and_preserves_decode_refusal() {
    assert!(ContactObservation::new(BODY_SECTOR_MASK).is_ok());
    assert!(ContactObservation::new(BODY_SECTOR_MASK + 1).is_err());
    assert!(matches!(
        ContactObservation::decode(&[BODY_SECTOR_MASK + 1]),
        Err(InfoDecodeError::ReservedValue {
            field: "active-body-sectors",
            actual: 32
        })
    ));
    assert!(matches!(
        ContactObservation::decode(&[]),
        Err(InfoDecodeError::WrongLength {
            expected: ROBOTICS_CONTACT_ENCODED_LEN,
            actual: 0,
        })
    ));
}
