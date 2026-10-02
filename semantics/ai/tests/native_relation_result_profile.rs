use conduit_ai::{
    RelationQueryMode, RelationResultProfile, RelationVariableIdentities, RelationVariableIdentity,
    SupportedRelationQuery,
};
use conduit_form::rust_binding::{BoundedSequence, NativeRustBinding};

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

#[test]
fn supported_relation_query_owns_finite_variables_and_positive_bounds() {
    let variables = |values: &[&str]| {
        let values = values
            .iter()
            .map(|value| RelationVariableIdentity::new((*value).into()).unwrap());
        RelationVariableIdentities::new(BoundedSequence::try_from_iter(values).unwrap()).unwrap()
    };
    let query = SupportedRelationQuery::new(
        variables(&["observation"]),
        variables(&["latent-state"]),
        RelationQueryMode::InferPosterior,
        RelationResultProfile::probabilistic(4).unwrap(),
        100,
        256,
    )
    .unwrap();
    let structured = query.clone().into_structured().unwrap();
    assert_eq!(
        SupportedRelationQuery::from_structured(structured).unwrap(),
        query
    );
    assert!(SupportedRelationQuery::new(
        variables(&["observation"]),
        variables(&["latent-state"]),
        RelationQueryMode::InferPosterior,
        RelationResultProfile::Deterministic,
        0,
        256,
    )
    .is_err());
    assert!(!include_str!("../src/relation.rs")
        .contains(concat!("pub struct ", "SupportedRelationQuery")));
}
