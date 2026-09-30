use conduit_ai::RelationResultProfile;
use conduit_form::rust_binding::NativeRustBinding;

fn assert_native_round_trip(value: RelationResultProfile) {
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        RelationResultProfile::from_structured(structured).unwrap(),
        value
    );
}

#[test]
fn relation_result_profile_round_trips_through_its_exact_native_payload_type() {
    assert_native_round_trip(RelationResultProfile::Deterministic);
    assert_native_round_trip(RelationResultProfile::probabilistic(4).unwrap());
}

#[test]
fn relation_result_profile_enforces_its_positive_sample_boundary() {
    assert!(RelationResultProfile::probabilistic(0).is_err());
    assert!(RelationResultProfile::probabilistic(u32::MAX).is_ok());
}
