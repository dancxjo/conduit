use conduit_ai::{
    HouseContextProvenanceClass, MechanismScore, RerankScore, RerankingProofClass,
    RetrievalMechanism,
};
use conduit_plot::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn retrieval_provenance_vocabularies_round_trip_through_their_native_types() {
    for provenance in [
        HouseContextProvenanceClass::ObservedSign,
        HouseContextProvenanceClass::DeclaredConfiguration,
        HouseContextProvenanceClass::ModelDerivedHistory,
    ] {
        assert_round_trip(provenance);
    }
    for mechanism in [
        RetrievalMechanism::VectorSimilarity,
        RetrievalMechanism::Lexical,
        RetrievalMechanism::Metadata,
        RetrievalMechanism::Temporal,
        RetrievalMechanism::DomainExact,
    ] {
        assert_round_trip(mechanism);
    }
    for proof in [
        RerankingProofClass::DeterministicConformance,
        RerankingProofClass::ModelDerived,
    ] {
        assert_round_trip(proof);
    }
    for score in [
        MechanismScore::SimilarityMicros(-9),
        MechanismScore::LexicalScore(4),
        MechanismScore::ExactMatch,
    ] {
        assert_round_trip(score);
    }
    for score in [RerankScore::HybridFusion(17), RerankScore::ModelDerived(-3)] {
        assert_round_trip(score);
    }
}
