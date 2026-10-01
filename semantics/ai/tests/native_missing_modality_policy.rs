use conduit_ai::{MissingModality, MissingModalityPolicy};
use conduit_form::rust_binding::{BoundedSequence, NativeRustBinding};

fn modality(value: &str) -> MissingModality {
    MissingModality::new(value.into()).unwrap()
}

fn assert_native_round_trip(value: MissingModalityPolicy) {
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        MissingModalityPolicy::from_structured(structured).unwrap(),
        value
    );
}

#[test]
fn missing_modality_policy_round_trips_through_its_exact_native_payload_type() {
    assert_native_round_trip(MissingModalityPolicy::Reject);

    let mut optional_modalities = BoundedSequence::new();
    optional_modalities.push(modality("audio")).unwrap();
    optional_modalities.push(modality("ema")).unwrap();
    let policy = MissingModalityPolicy::permit_declared(optional_modalities).unwrap();
    let MissingModalityPolicy::PermitDeclared(payload) = &policy else {
        panic!("checked declared-modality construction must retain its payload");
    };
    assert_eq!(
        payload
            .optional_modalities()
            .iter()
            .map(|value| value.get().as_str())
            .collect::<Vec<_>>(),
        ["audio", "ema"]
    );
    assert_native_round_trip(policy);
}

#[test]
fn missing_modality_and_sequence_enforce_exact_native_bounds() {
    assert!(MissingModality::new("x".repeat(128)).is_ok());
    assert!(MissingModality::new(String::new()).is_err());
    assert!(MissingModality::new("x".repeat(129)).is_err());

    let mut optional_modalities = BoundedSequence::new();
    for index in 0..32 {
        optional_modalities
            .push(modality(&format!("modality-{index}")))
            .unwrap();
    }
    assert!(optional_modalities.push(modality("overflow")).is_err());
    assert!(MissingModalityPolicy::permit_declared(optional_modalities).is_ok());
}
