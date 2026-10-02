use conduit_ai::{ChunkIdentity, RerankObservation, RerankingProofClass, RerankingStrategy};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn reranking_strategy_payload_is_one_bounded_native_type() {
    let observation = RerankObservation::new(ChunkIdentity::from_digest([7; 32]), -25, 1).unwrap();
    let structured = observation.into_structured().unwrap();
    assert_eq!(
        RerankObservation::from_structured(structured).unwrap(),
        observation
    );
    assert!(RerankObservation::new(ChunkIdentity::from_digest([7; 32]), 0, 0).is_err());
    assert!(
        !include_str!("../src/reranking.rs").contains(concat!("pub struct ", "RerankObservation"))
    );

    let observed = RerankingStrategy::observed_scores(
        RerankingProofClass::ModelDerived,
        "scoring-run/7".into(),
    )
    .unwrap();
    let RerankingStrategy::ObservedScores(payload) = &observed else {
        panic!("observed scoring retains its exact payload")
    };
    assert_eq!(*payload.proof_class(), RerankingProofClass::ModelDerived);
    assert_eq!(payload.scoring_run_identity(), "scoring-run/7");
    let structured = observed.clone().into_structured().unwrap();
    assert_eq!(
        structured.value_type(),
        &RerankingStrategy::semantic_type().unwrap()
    );
    assert_eq!(
        RerankingStrategy::from_structured(structured).unwrap(),
        observed
    );

    assert!(
        RerankingStrategy::observed_scores(RerankingProofClass::ModelDerived, String::new(),)
            .is_err()
    );
    assert!(
        RerankingStrategy::observed_scores(RerankingProofClass::ModelDerived, "x".repeat(257),)
            .is_err()
    );

    let preserved = RerankingStrategy::PreserveHybridFusion;
    let structured = preserved.clone().into_structured().unwrap();
    assert_eq!(
        RerankingStrategy::from_structured(structured).unwrap(),
        preserved
    );
}
