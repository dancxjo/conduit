use conduit_ai::{
    canonical_hit_order, ClockBasis, CompatibleMetrics, Embedding, EmbeddingNormalization,
    EmbeddingProfile, MetadataFilter, SimilarityHit, SimilarityMetric, SimilarityQuery,
    SimilarityScore, SimilarityThreshold, TemporalProvenance, TemporalRetrievalIntent,
    VectorMetadata, VectorRecord, VectorRefusal, MAXIMUM_SIMILARITY_TOP_K,
};

fn profile(normalization: EmbeddingNormalization) -> EmbeddingProfile {
    EmbeddingProfile {
        identity: "embedding/profile-1".into(),
        semantic_space_identity: "space/document-meaning-v1".into(),
        model_identity: "model/fixture-3d-v1".into(),
        provider_identity: "provider/reviewed-fixture-v1".into(),
        dimensions: 3,
        normalization,
        compatible_metrics: CompatibleMetrics {
            cosine_similarity: true,
            dot_product_similarity: true,
            squared_euclidean_distance: true,
        },
    }
}

fn embedding(values: [f32; 3]) -> Embedding {
    Embedding {
        profile: profile(EmbeddingNormalization::None),
        values: values.into(),
    }
}

fn query(metric: SimilarityMetric) -> SimilarityQuery {
    SimilarityQuery {
        embedding: embedding([1.0, 2.0, 2.0]),
        metric,
        top_k: 4,
        threshold: None,
        filters: vec![MetadataFilter::Equal {
            key: "language".into(),
            value: "en".into(),
        }],
        temporal_intent: Some(TemporalRetrievalIntent::LatestEvidence),
    }
}

fn provenance() -> TemporalProvenance {
    TemporalProvenance {
        event_at: Some(100),
        valid_from: Some(100),
        valid_until: None,
        observed_at: Some(110),
        recorded_at: Some(120),
        ingested_at: Some(130),
        retrieved_at: 140,
        reference_at: 150,
        clock_basis: ClockBasis::UnixEpochMilliseconds,
        uncertainty_millis: None,
    }
}

#[test]
fn exact_cosine_dot_and_squared_l2_semantics_are_distinct() {
    let candidate = embedding([2.0, 0.0, 1.0]);
    assert_eq!(
        query(SimilarityMetric::DotProductSimilarity).score(&candidate),
        Ok(SimilarityScore::Similarity(4.0))
    );
    assert_eq!(
        query(SimilarityMetric::SquaredEuclideanDistance).score(&candidate),
        Ok(SimilarityScore::SquaredDistance(6.0))
    );
    let SimilarityScore::Similarity(cosine) = query(SimilarityMetric::CosineSimilarity)
        .score(&candidate)
        .unwrap()
    else {
        panic!("cosine has similarity semantics")
    };
    assert!((cosine - 4.0 / (9.0_f32 * 5.0).sqrt()).abs() < 0.000_001);
}

#[test]
fn same_dimensions_do_not_make_spaces_models_or_providers_compatible() {
    let base = profile(EmbeddingNormalization::None);
    for (changed, expected) in [
        (base.clone(), VectorRefusal::SemanticSpaceMismatch),
        (base.clone(), VectorRefusal::ModelMismatch),
        (base.clone(), VectorRefusal::ProviderMismatch),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, pair)| {
        let (mut changed, refusal) = pair;
        match index {
            0 => changed.semantic_space_identity = "space/other".into(),
            1 => changed.model_identity = "model/other".into(),
            _ => changed.provider_identity = "provider/other".into(),
        }
        (changed, refusal)
    }) {
        assert_eq!(
            base.compatibility(&changed, SimilarityMetric::CosineSimilarity),
            Err(expected)
        );
    }
}

#[test]
fn dimensions_and_metric_admission_are_exact_profile_compatibility() {
    let base = profile(EmbeddingNormalization::None);
    let mut dimensions = base.clone();
    dimensions.dimensions = 4;
    assert_eq!(
        base.compatibility(&dimensions, SimilarityMetric::CosineSimilarity),
        Err(VectorRefusal::DimensionMismatch)
    );

    let mut metrics = base.clone();
    metrics.compatible_metrics.cosine_similarity = false;
    assert_eq!(
        base.compatibility(&metrics, SimilarityMetric::CosineSimilarity),
        Err(VectorRefusal::MetricNotCompatible)
    );
}

#[test]
fn normalization_and_zero_vector_laws_fail_closed() {
    let unit = Embedding {
        profile: profile(EmbeddingNormalization::UnitLength),
        values: vec![1.0, 1.0, 0.0],
    };
    assert_eq!(unit.validate(), Err(VectorRefusal::NormalizationMismatch));
    assert_eq!(
        query(SimilarityMetric::CosineSimilarity).score(&embedding([0.0, 0.0, 0.0])),
        Err(VectorRefusal::ZeroVector)
    );
}

#[test]
fn top_k_threshold_filters_and_temporal_intent_are_finite_and_typed() {
    let mut bounded = query(SimilarityMetric::DotProductSimilarity);
    bounded.top_k = MAXIMUM_SIMILARITY_TOP_K + 1;
    assert_eq!(bounded.validate(), Err(VectorRefusal::TopKTooLarge));

    let mut thresholded = query(SimilarityMetric::SquaredEuclideanDistance);
    thresholded.threshold = Some(SimilarityThreshold::MinimumSimilarity(0.5));
    assert_eq!(
        thresholded.validate(),
        Err(VectorRefusal::ThresholdMetricMismatch)
    );

    let mut temporal = query(SimilarityMetric::DotProductSimilarity);
    temporal.temporal_intent = Some(TemporalRetrievalIntent::EvidenceWithin { start: 2, end: 1 });
    assert_eq!(
        temporal.validate(),
        Err(VectorRefusal::InvalidTemporalIntent)
    );
}

#[test]
fn records_and_hits_preserve_exact_source_resource_and_temporal_provenance() {
    let record = VectorRecord {
        value: "bounded source value",
        embedding: embedding([1.0, 0.0, 0.0]),
        source_identity: "sign/source-7".into(),
        resource_identity: "resource/document-9".into(),
        metadata: vec![VectorMetadata {
            key: "language".into(),
            value: "en".into(),
        }],
        temporal_provenance: Some(provenance()),
    };
    record.validate().unwrap();
    let hit = SimilarityHit {
        value: record.value,
        score: SimilarityScore::Similarity(0.75),
        rank: 1,
        index_generation: 12,
        source_identity: record.source_identity,
        resource_identity: record.resource_identity,
        temporal_provenance: record.temporal_provenance,
    };
    hit.validate_for(&query(SimilarityMetric::CosineSimilarity))
        .unwrap();
    assert_eq!(hit.source_identity, "sign/source-7");
    assert_eq!(hit.temporal_provenance.unwrap().event_at, Some(100));
}

#[test]
fn thresholds_do_not_turn_similarity_into_probability_or_truth() {
    let mut query = query(SimilarityMetric::CosineSimilarity);
    query.threshold = Some(SimilarityThreshold::MinimumSimilarity(0.8));
    assert_eq!(
        query.admits_score(SimilarityScore::Similarity(0.79)),
        Ok(false)
    );
    assert_eq!(
        query.admits_score(SimilarityScore::SquaredDistance(0.1)),
        Err(VectorRefusal::ThresholdMetricMismatch)
    );
    let encoded = serde_json::to_string(&SimilarityScore::Similarity(0.9)).unwrap();
    assert!(!encoded.contains("probability"));
    assert!(!encoded.contains("truth"));
}

#[test]
fn equal_scores_have_canonical_source_then_resource_order() {
    let hit = |source: &str, resource: &str| SimilarityHit {
        value: (),
        score: SimilarityScore::Similarity(0.5),
        rank: 1,
        index_generation: 1,
        source_identity: source.into(),
        resource_identity: resource.into(),
        temporal_provenance: None,
    };
    assert!(canonical_hit_order(&hit("a", "z"), &hit("b", "a")).is_lt());
    assert!(canonical_hit_order(&hit("a", "a"), &hit("a", "b")).is_lt());
}

#[test]
fn canonical_serialization_is_stable_and_contains_no_storage_engine_detail() {
    let profile = profile(EmbeddingNormalization::None);
    let first = serde_json::to_string(&profile).unwrap();
    let second = serde_json::to_string(&profile).unwrap();
    assert_eq!(first, second);
    for forbidden in ["database", "hnsw", "ivf", "postgres", "sqlite"] {
        assert!(!first.contains(forbidden));
    }
}
