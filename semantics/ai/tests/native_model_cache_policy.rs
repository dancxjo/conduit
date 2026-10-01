use conduit_ai::ModelCachePolicy;
use conduit_form::rust_binding::NativeRustBinding;

fn assert_native_round_trip(value: ModelCachePolicy) {
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        ModelCachePolicy::from_structured(structured).unwrap(),
        value
    );
}

#[test]
fn model_cache_policy_round_trips_through_its_exact_native_payload_type() {
    assert_native_round_trip(ModelCachePolicy::NoCache);
    let bounded = ModelCachePolicy::bounded(4096, 2).unwrap();
    let ModelCachePolicy::Bounded(payload) = &bounded else {
        panic!("checked bounded construction must retain its payload");
    };
    assert_eq!(*payload.maximum_loaded_models(), 2);
    assert_eq!(*payload.maximum_loaded_bytes(), 4096);
    assert_native_round_trip(bounded);
}

#[test]
fn model_cache_policy_enforces_positive_model_and_byte_bounds() {
    assert!(ModelCachePolicy::bounded(1, 0).is_err());
    assert!(ModelCachePolicy::bounded(0, 1).is_err());
    assert!(ModelCachePolicy::bounded(u64::MAX, u16::MAX).is_ok());
}
