use conduit_ai::{RerankingProofClass, RerankingStrategy};
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn reranking_strategy_payload_is_one_bounded_native_type() {
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
