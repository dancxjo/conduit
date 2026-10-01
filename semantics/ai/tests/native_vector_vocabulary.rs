use conduit_ai::{
    EmbeddingNormalization, ExactVectorSearchRefusal, SimilarityMetric, SimilarityScore,
    TemporalEvidenceSelectionRefusal, VectorIndexResourceRefusal, VectorRefusal,
};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn similarity_scores_round_trip_exact_finite_ieee_values() {
    for score in [
        SimilarityScore::from_similarity(0.0).unwrap(),
        SimilarityScore::from_similarity(-0.0).unwrap(),
        SimilarityScore::from_similarity(f32::MAX).unwrap(),
        SimilarityScore::from_squared_distance(-1.0).unwrap(),
        SimilarityScore::from_squared_distance(f32::MIN_POSITIVE).unwrap(),
    ] {
        assert_round_trip(score);
    }

    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            SimilarityScore::from_similarity(value),
            Err(VectorRefusal::NonFiniteScore)
        );
        assert_eq!(
            SimilarityScore::from_squared_distance(value),
            Err(VectorRefusal::NonFiniteScore)
        );
    }
}

#[test]
fn vector_vocabularies_round_trip_through_their_native_types() {
    for metric in [
        SimilarityMetric::CosineSimilarity,
        SimilarityMetric::DotProductSimilarity,
        SimilarityMetric::SquaredEuclideanDistance,
    ] {
        assert_round_trip(metric);
    }
    for normalization in [
        EmbeddingNormalization::None,
        EmbeddingNormalization::UnitLength,
    ] {
        assert_round_trip(normalization);
    }
    for refusal in [
        VectorRefusal::EmptyIdentity,
        VectorRefusal::DimensionMismatch,
        VectorRefusal::RankExceedsTopK,
    ] {
        assert_round_trip(refusal);
    }
    for refusal in [
        ExactVectorSearchRefusal::Vector(VectorRefusal::DimensionMismatch),
        ExactVectorSearchRefusal::Resource(VectorIndexResourceRefusal::ResourceBusy),
        ExactVectorSearchRefusal::Temporal(TemporalEvidenceSelectionRefusal::ReferenceMismatch),
        ExactVectorSearchRefusal::EarlierHistoryRequired,
    ] {
        assert_round_trip(refusal);
    }
}
