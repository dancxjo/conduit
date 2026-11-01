#![cfg(feature = "face-projection")]
//! Representation refusal proof, not authenticated history or storage evidence.
use conduit_core::{Quantity, Unit};
use conduit_environment_runtime::{
    observed_record::{ObservedRecord, Refusal, BYTES},
    recording::RecordingDisposition as D,
};

fn available() -> ObservedRecord {
    ObservedRecord {
        sequence: 19,
        observed_at_boot_ms: 9012,
        disposition: D::Available,
        quantities: Some((
            Quantity::new(68, Unit::Fahrenheit),
            Quantity::new(55, Unit::Percent),
        )),
    }
}

#[test]
fn exact_forms_preserve_clock_sequence_and_semantic_quantities() {
    let bytes = available().encode().unwrap();
    assert_eq!(bytes.len(), BYTES);
    let record = ObservedRecord::decode(&bytes).unwrap();
    assert_eq!((record.sequence, record.observed_at_boot_ms), (19, 9012));
    assert_eq!(record.disposition, D::Available);
    assert_eq!(
        record.quantities,
        Some((
            Quantity::new(20, Unit::Celsius),
            Quantity::new(55, Unit::Percent)
        ))
    );
    assert_eq!(record.encode().unwrap(), bytes);
    let mut inexact = available();
    inexact.quantities = Some((
        Quantity::from_decimal(2001, -2, Unit::Celsius).unwrap(),
        Quantity::new(55, Unit::Percent),
    ));
    assert_eq!(inexact.encode(), Err(Refusal::Quantity));
}

#[test]
fn malformed_records_cannot_turn_absence_or_failure_into_a_reading() {
    let bytes = available().encode().unwrap();
    for len in [0, 1, BYTES - 1] {
        assert!(matches!(
            ObservedRecord::decode(&bytes[..len]),
            Err(Refusal::Noncanonical)
        ));
    }
    let mut invalid = bytes;
    invalid[..8].fill(0);
    assert!(matches!(
        ObservedRecord::decode(&invalid),
        Err(Refusal::Sequence)
    ));
    invalid = bytes;
    invalid[16] = 255;
    assert!(matches!(
        ObservedRecord::decode(&invalid),
        Err(Refusal::Disposition)
    ));
    let failure = ObservedRecord {
        sequence: 20,
        observed_at_boot_ms: 9050,
        disposition: D::CaptureTimeout,
        quantities: None,
    };
    let mut encoded = failure.encode().unwrap();
    assert_eq!(ObservedRecord::decode(&encoded).unwrap().quantities, None);
    encoded[17] = 1;
    assert!(matches!(
        ObservedRecord::decode(&encoded),
        Err(Refusal::Noncanonical)
    ));
    let mut missing = available();
    missing.quantities = None;
    assert_eq!(missing.encode(), Err(Refusal::Noncanonical));
    let mut false_failure = available();
    false_failure.disposition = D::CaptureTimeout;
    assert_eq!(false_failure.encode(), Err(Refusal::Noncanonical));
}
