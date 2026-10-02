use conduit_ai::{
    FusionStrategy, GroundedAnswerBytePage, GroundedAnswerBytePages, GroundedAnswerDisposition,
    HybridFusionPolicy, LlmDeterminismProfile, MechanismScore, ModelDerivedResult,
    ModelResultDisposition, ModelResultPayload, ModelResultProvenance, ModelWorkAccounting,
    PortableGroundedAnswer, PortableModelDerivedResult, RetrievalContribution, RetrievalMechanism,
    RetrieverIdentity,
};
use conduit_form::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};

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
        "type RerankedCandidate<T> =",
        "type RerankingReceipt<T> =",
        "type ContextTemporalEvidence<TContext> =",
        "type ContextCandidate<T, TContext> =",
        "type SelectedContextItem<T, TContext> =",
        "type StructuredContext<T, TContext> =",
        "type GroundedAnswerRequest<T, TContext> =",
        "type ProposedClaimSupport<TCitation> =",
        "type ProposedGroundedClaim<TCitation> =",
        "type GroundedAnswer<TAnswer, TCitation> =",
        "type ModelDerivedResult<TPayload> =",
    ] {
        assert!(source.contains(declaration), "missing {declaration}");
    }
}

#[test]
fn model_and_grounded_outputs_have_exact_native_specializations() {
    let payload = ModelResultPayload::new(BoundedBytes::new(b"answer").unwrap()).unwrap();
    let result = PortableModelDerivedResult::new(
        ModelWorkAccounting::new(0, 0, 0, 6, 1).unwrap(),
        None,
        LlmDeterminismProfile::DeterministicValidationFixture,
        ModelResultDisposition::Produced,
        "model/local".into(),
        payload,
        "value/text".into(),
        ModelResultProvenance::ModelDerived,
        "request/1".into(),
        "run/1".into(),
    )
    .unwrap();
    let structured = result.clone().into_structured().unwrap();
    assert_eq!(
        PortableModelDerivedResult::from_structured(structured).unwrap(),
        result
    );

    let page = GroundedAnswerBytePage::new(BoundedBytes::new(b"answer").unwrap()).unwrap();
    let pages =
        GroundedAnswerBytePages::new(BoundedSequence::try_from_iter([page]).unwrap()).unwrap();
    let answer = PortableGroundedAnswer::new(
        pages,
        "value/text".into(),
        BoundedSequence::new(),
        BoundedSequence::new(),
        "context/default".into(),
        GroundedAnswerDisposition::InsufficientEvidence,
        BoundedSequence::new(),
        "model/local".into(),
        "run/1".into(),
        "grounding/default".into(),
        ModelResultProvenance::ModelDerived,
        "request/1".into(),
    )
    .unwrap();
    let structured = answer.clone().into_structured().unwrap();
    assert_eq!(
        PortableGroundedAnswer::from_structured(structured).unwrap(),
        answer
    );

    assert!(BoundedSequence::<GroundedAnswerBytePage, 4>::try_from_iter(
        (0..5).map(|_| { GroundedAnswerBytePage::new(BoundedBytes::new(&[]).unwrap()).unwrap() })
    )
    .is_err());
    assert!(include_str!("../src/model_result.rs")
        .contains("authored `ModelDerivedResult<ModelResultPayload>`"));
    assert!(include_str!("../src/grounded_answer.rs")
        .contains("GroundedAnswer<GroundedAnswerBytePages, Citation>"));
    let _compatibility_carrier: Option<(ModelDerivedResult, conduit_ai::GroundedAnswer)> = None;
}
