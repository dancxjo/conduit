use conduit_ai::{
    exact_vector_search, ClockBasis, CompatibleMetrics, Embedding, EmbeddingNormalization,
    EmbeddingProfile, EntityBoundary, ExactVectorSearchCandidate, ExactVectorSearchRefusal,
    MetadataFilter, SimilarityMetric, SimilarityQuery, SimilarityScore, SimilarityThreshold,
    TemporalProvenance, TemporalRetrievalIntent, TemporalSource, TemporalValidity,
    TransitionDirection, VectorIndexAuthority, VectorIndexAuthorization, VectorIndexBounds,
    VectorIndexContract, VectorIndexMutation, VectorIndexQueryAdmission,
    VectorIndexResourceRefusal, VectorIndexState, VectorMetadata, VectorRecord,
    VectorSearchProofClass, VECTOR_INDEX_RESOURCE_CLASS,
};
use conduit_core::{ResourceBinding, ResourceClassId, ResourcePoolId};

fn profile() -> EmbeddingProfile {
    EmbeddingProfile {
        identity: "embedding/exact-fixture".into(),
        semantic_space_identity: "space/exact-fixture".into(),
        model_identity: "model/exact-fixture".into(),
        provider_identity: "provider/exact-fixture".into(),
        dimensions: 3,
        normalization: EmbeddingNormalization::None,
        compatible_metrics: CompatibleMetrics {
            cosine_similarity: true,
            dot_product_similarity: true,
            squared_euclidean_distance: true,
        },
    }
}

fn state(sources: &[&str]) -> VectorIndexState {
    let mut state = VectorIndexState::new(
        VectorIndexContract {
            index_identity: "index/exact-fixture".into(),
            generation: 19,
            embedding_profile: profile(),
            pool_id: ResourcePoolId::from("pool/exact-fixture"),
            class_id: ResourceClassId::from(VECTOR_INDEX_RESOURCE_CLASS),
            bounds: VectorIndexBounds {
                maximum_items: 16,
                maximum_storage_bytes: 4_096,
                maximum_query_work_units: 64,
                maximum_results: 8,
                maximum_concurrent_queries: 1,
            },
        },
        vec![VectorIndexAuthorization {
            authority_identity: "authority/exact-query".into(),
            authority: VectorIndexAuthority {
                query: true,
                insert: true,
                upsert: false,
                delete: false,
                maintain: false,
            },
        }],
    )
    .unwrap();
    for source in sources {
        let handle = state.handle("authority/exact-query").unwrap();
        state
            .mutate(
                &handle,
                VectorIndexMutation::Insert {
                    mutation_identity: format!("mutation/{source}"),
                    source_identity: (*source).into(),
                    stored_bytes: 1,
                },
            )
            .unwrap();
    }
    state
}

fn binding(work_units: u32) -> ResourceBinding {
    ResourceBinding {
        content: None,
        pool_id: ResourcePoolId::from("pool/exact-fixture"),
        class_id: ResourceClassId::from(VECTOR_INDEX_RESOURCE_CLASS),
        units: work_units,
        protected: None,
        compute: None,
    }
}

fn admission(work_units: u32, maximum_results: u32) -> VectorIndexQueryAdmission {
    VectorIndexQueryAdmission {
        work_units,
        maximum_results,
        concurrent_queries: 1,
    }
}

fn query(metric: SimilarityMetric, top_k: u32) -> SimilarityQuery {
    SimilarityQuery {
        embedding: Embedding {
            profile: profile(),
            values: vec![1.0, 0.0, 0.0],
        },
        metric,
        top_k,
        threshold: None,
        filters: vec![],
        temporal_intent: None,
    }
}

fn provenance(event_at: u64, valid_until: Option<u64>) -> TemporalProvenance {
    TemporalProvenance {
        event_at: Some(event_at),
        valid_from: Some(event_at),
        valid_until,
        observed_at: Some(event_at + 1),
        recorded_at: Some(event_at + 2),
        ingested_at: Some(event_at + 3),
        retrieved_at: 900,
        reference_at: 1_000,
        clock_basis: ClockBasis::UnixEpochMilliseconds,
        uncertainty_millis: None,
    }
}

fn candidate(
    source: &'static str,
    resource: &str,
    values: [f32; 3],
    event_at: u64,
) -> ExactVectorSearchCandidate<&'static str> {
    ExactVectorSearchCandidate {
        record: VectorRecord {
            value: source,
            embedding: Embedding {
                profile: profile(),
                values: values.into(),
            },
            source_identity: source.into(),
            resource_identity: resource.into(),
            metadata: vec![
                VectorMetadata {
                    key: "language".into(),
                    value: "en".into(),
                },
                VectorMetadata {
                    key: "kind".into(),
                    value: if source.ends_with('a') { "note" } else { "log" }.into(),
                },
            ],
            temporal_provenance: Some(provenance(event_at, None)),
        },
        temporal_source: TemporalSource::Event,
        boundary: None,
        transition: None,
        validity: TemporalValidity::Current,
    }
}

fn search(
    query: &SimilarityQuery,
    candidates: &[ExactVectorSearchCandidate<&'static str>],
) -> Result<conduit_ai::ExactVectorSearchResult<&'static str>, ExactVectorSearchRefusal> {
    let sources: Vec<_> = candidates
        .iter()
        .map(|candidate| candidate.record.source_identity.as_str())
        .collect();
    let state = state(&sources);
    let handle = state.handle("authority/exact-query").unwrap();
    exact_vector_search(
        &state,
        &handle,
        query,
        candidates,
        admission(64, query.top_k),
        &binding(64),
        true,
    )
}

#[test]
fn exact_oracle_defines_all_metric_orders_and_exact_generation() {
    let candidates = vec![
        candidate("source/a", "resource/z", [1.0, 0.0, 0.0], 100),
        candidate("source/b", "resource/y", [0.5, 0.5, 0.0], 200),
        candidate("source/c", "resource/x", [-1.0, 0.0, 0.0], 300),
    ];
    for metric in [
        SimilarityMetric::CosineSimilarity,
        SimilarityMetric::DotProductSimilarity,
        SimilarityMetric::SquaredEuclideanDistance,
    ] {
        let result = search(&query(metric, 3), &candidates).unwrap();
        assert_eq!(
            result.proof_class,
            VectorSearchProofClass::DeterministicExactOracle
        );
        assert_eq!(result.index_generation, 22);
        assert_eq!(result.candidate_count, 3);
        assert_eq!(result.hits[0].source_identity, "source/a");
        assert_eq!(
            result.hits.iter().map(|hit| hit.rank).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }
}

#[test]
fn filters_threshold_top_k_and_equal_score_ties_are_canonical() {
    let candidates = vec![
        candidate("source/b", "resource/a", [1.0, 0.0, 0.0], 100),
        candidate("source/a", "resource/z", [1.0, 0.0, 0.0], 200),
        candidate("source/c", "resource/c", [0.2, 0.0, 0.0], 300),
    ];
    let mut query = query(SimilarityMetric::DotProductSimilarity, 2);
    query.filters = vec![
        MetadataFilter::Present {
            key: "language".into(),
        },
        MetadataFilter::Equal {
            key: "kind".into(),
            value: "note".into(),
        },
    ];
    query.threshold = Some(SimilarityThreshold::MinimumSimilarity(0.5));
    let result = search(&query, &candidates).unwrap();
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].source_identity, "source/a");

    query.filters.clear();
    let result = search(&query, &candidates).unwrap();
    assert_eq!(result.hits.len(), 2);
    assert_eq!(result.hits[0].source_identity, "source/a");
    assert_eq!(result.hits[1].source_identity, "source/b");
}

#[test]
fn temporal_intents_select_exact_portable_evidence_before_scoring() {
    let mut candidates = vec![
        candidate("source/a", "resource/a", [0.1, 0.0, 0.0], 100),
        candidate("source/b", "resource/b", [0.9, 0.0, 0.0], 200),
        candidate("source/c", "resource/c", [0.5, 0.0, 0.0], 300),
    ];
    candidates[0].boundary = Some(EntityBoundary::Started);
    candidates[1].transition = Some(TransitionDirection::IntoState);
    candidates[1]
        .record
        .temporal_provenance
        .as_mut()
        .unwrap()
        .valid_until = Some(250);

    for (intent, expected) in [
        (TemporalRetrievalIntent::EarliestEvidence, vec!["source/a"]),
        (TemporalRetrievalIntent::LatestEvidence, vec!["source/c"]),
        (
            TemporalRetrievalIntent::EventOrdering,
            vec!["source/b", "source/c", "source/a"],
        ),
        (
            TemporalRetrievalIntent::StateValidAt { instant: 225 },
            vec!["source/b", "source/a"],
        ),
        (
            TemporalRetrievalIntent::Transition {
                direction: TransitionDirection::IntoState,
            },
            vec!["source/b"],
        ),
        (
            TemporalRetrievalIntent::DurationSince {
                boundary: EntityBoundary::Started,
            },
            vec!["source/a"],
        ),
        (
            TemporalRetrievalIntent::EvidenceWithin {
                start: 150,
                end: 250,
            },
            vec!["source/b"],
        ),
    ] {
        let mut query = query(SimilarityMetric::DotProductSimilarity, 3);
        query.temporal_intent = Some(intent);
        let result = search(&query, &candidates).unwrap();
        assert_eq!(
            result
                .hits
                .iter()
                .map(|hit| hit.source_identity.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn full_scan_work_is_admitted_before_filters_or_scores() {
    let candidates = vec![
        candidate("source/a", "resource/a", [1.0, 0.0, 0.0], 100),
        candidate("source/b", "resource/b", [0.0, 1.0, 0.0], 200),
    ];
    let mut query = query(SimilarityMetric::DotProductSimilarity, 2);
    query.filters.push(MetadataFilter::Equal {
        key: "language".into(),
        value: "never-matches".into(),
    });
    let state = state(&["source/a", "source/b"]);
    let handle = state.handle("authority/exact-query").unwrap();
    assert_eq!(
        exact_vector_search(
            &state,
            &handle,
            &query,
            &candidates,
            admission(5, 2),
            &binding(5),
            true,
        ),
        Err(ExactVectorSearchRefusal::Resource(
            VectorIndexResourceRefusal::QueryWorkLimitExceeded
        ))
    );
}

#[test]
fn temporal_history_and_provenance_gaps_refuse_distinctly() {
    let candidates = vec![candidate("source/a", "resource/a", [1.0, 0.0, 0.0], 100)];
    let mut query = query(SimilarityMetric::CosineSimilarity, 1);
    query.temporal_intent = Some(TemporalRetrievalIntent::EarliestEvidence);
    let state = state(&["source/a"]);
    let handle = state.handle("authority/exact-query").unwrap();
    assert_eq!(
        exact_vector_search(
            &state,
            &handle,
            &query,
            &candidates,
            admission(3, 1),
            &binding(3),
            false,
        ),
        Err(ExactVectorSearchRefusal::EarlierHistoryRequired)
    );

    let mut missing = candidates;
    missing[0].record.temporal_provenance = None;
    assert_eq!(
        exact_vector_search(
            &state,
            &handle,
            &query,
            &missing,
            admission(3, 1),
            &binding(3),
            true,
        ),
        Err(ExactVectorSearchRefusal::TemporalProvenanceRequired)
    );
}

#[test]
fn index_membership_and_result_admission_are_exact() {
    let candidates = vec![candidate("source/a", "resource/a", [1.0, 0.0, 0.0], 100)];
    let one_query = query(SimilarityMetric::DotProductSimilarity, 1);
    let wrong_state = state(&["source/different"]);
    let handle = wrong_state.handle("authority/exact-query").unwrap();
    assert_eq!(
        exact_vector_search(
            &wrong_state,
            &handle,
            &one_query,
            &candidates,
            admission(3, 1),
            &binding(3),
            true,
        ),
        Err(ExactVectorSearchRefusal::IndexMembershipMismatch)
    );

    let state = state(&["source/a"]);
    let handle = state.handle("authority/exact-query").unwrap();
    let query = query(SimilarityMetric::DotProductSimilarity, 2);
    assert_eq!(
        exact_vector_search(
            &state,
            &handle,
            &query,
            &candidates,
            admission(3, 1),
            &binding(3),
            true,
        ),
        Err(ExactVectorSearchRefusal::Resource(
            VectorIndexResourceRefusal::ResultLimitExceeded
        ))
    );
}

#[test]
fn score_values_remain_metric_values_not_probabilities() {
    let result = search(
        &query(SimilarityMetric::SquaredEuclideanDistance, 1),
        &[candidate("source/a", "resource/a", [0.0, 1.0, 0.0], 100)],
    )
    .unwrap();
    assert_eq!(result.hits[0].score, SimilarityScore::SquaredDistance(2.0));
}
