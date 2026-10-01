use conduit_ai::ModelDimensionConstraint;
use conduit_form::rust_binding::NativeRustBinding;

fn assert_native_round_trip(value: ModelDimensionConstraint) {
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        ModelDimensionConstraint::from_structured(structured).unwrap(),
        value
    );
}

#[test]
fn model_dimension_constraint_round_trips_exact_payloads() {
    assert_native_round_trip(ModelDimensionConstraint::fixed(12).unwrap());
    let bounded = ModelDimensionConstraint::bounded(256, 1).unwrap();
    let ModelDimensionConstraint::Bounded(payload) = &bounded else {
        panic!("checked bounded construction must retain its payload");
    };
    assert_eq!(*payload.minimum(), 1);
    assert_eq!(*payload.maximum(), 256);
    assert_native_round_trip(bounded);
}

#[test]
fn model_dimension_constraint_enforces_positive_native_boundaries() {
    assert!(ModelDimensionConstraint::fixed(0).is_err());
    assert!(ModelDimensionConstraint::fixed(1).is_ok());
    assert!(ModelDimensionConstraint::fixed(u64::MAX).is_ok());
    assert!(ModelDimensionConstraint::bounded(1, 0).is_err());
    assert!(ModelDimensionConstraint::bounded(0, 1).is_err());
    assert!(ModelDimensionConstraint::bounded(u64::MAX, 1).is_ok());
}
