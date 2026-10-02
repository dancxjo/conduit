use conduit_ai::{
    FusionStrategy, HybridFusionPolicy, MechanismScore, RetrievalContribution, RetrievalMechanism,
    RetrieverIdentity,
};
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn retrieval_contribution_is_native_and_exactly_bounded() {
    let contribution = RetrievalContribution {
        retriever: RetrieverIdentity::new(
            "retriever/vector-primary".into(),
            RetrievalMechanism::VectorSimilarity,
        )
        .unwrap(),
        stage_rank: 1,
        score: Some(MechanismScore::SimilarityMicros(875_000)),
        temporal_evidence_identity: None,
    };
    let structured = contribution.clone().into_structured().unwrap();
    assert_eq!(
        RetrievalContribution::from_structured(structured).unwrap(),
        contribution
    );
    assert!(RetrievalContribution::new(
        contribution.retriever.clone(),
        contribution.score,
        0,
        None,
    )
    .is_err());
    assert!(!include_str!("../src/hybrid_retrieval.rs")
        .contains(concat!("pub struct ", "RetrievalContribution")));
}

#[test]
fn hybrid_policy_is_native_and_owns_its_finite_envelope() {
    let policy = HybridFusionPolicy::from_parts(
        "fusion/native-rrf@1".into(),
        FusionStrategy::reciprocal_rank(60).unwrap(),
        vec![
            RetrievalMechanism::VectorSimilarity,
            RetrievalMechanism::Lexical,
        ],
        None,
        1_024,
        1_024,
        1_048_576,
    )
    .unwrap();
    let structured = policy.clone().into_structured().unwrap();
    assert_eq!(
        HybridFusionPolicy::from_structured(structured).unwrap(),
        policy
    );
    assert!(HybridFusionPolicy::from_parts(
        "fusion/invalid@1".into(),
        FusionStrategy::reciprocal_rank(60).unwrap(),
        vec![],
        None,
        0,
        1,
        1,
    )
    .is_err());
    assert!(!include_str!("../src/hybrid_retrieval.rs")
        .contains(concat!("pub struct ", "HybridFusionPolicy")));
}

#[test]
fn generic_retrieval_carriers_have_one_authored_conduitese_family() {
    let source = include_str!("../types.conduit");
    for declaration in [
        "type Chunk<T> =",
        "type Candidate<T> =",
        "type ContextItem<T> =",
        "type ContextSelection<T> =",
        "type StageCandidate<T> =",
        "type RetrievalStage<T> =",
        "type HybridCandidate<T> =",
        "type HybridRetrievalOutcome<T> =",
        "type ExactVectorSearchCandidate<T> =",
        "type ExactVectorSearchResult<T> =",
        "type VectorSearchValue<T> =",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
}
