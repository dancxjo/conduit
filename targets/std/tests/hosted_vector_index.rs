use conduit_ai::{
    exact_vector_search, ClockBasis, CompatibleMetrics, Embedding, EmbeddingNormalization,
    EmbeddingProfile, ExactVectorSearchCandidate, MetadataFilter, SimilarityMetric,
    SimilarityQuery, TemporalProvenance, TemporalRetrievalIntent, TemporalSource, TemporalValidity,
    VectorIndexAuthority, VectorIndexAuthorization, VectorIndexBounds, VectorIndexContract,
    VectorIndexQueryAdmission, VectorIndexResourceRefusal, VectorIndexState, VectorMetadata,
    VectorRecord, VECTOR_INDEX_RESOURCE_CLASS,
};
use conduit_core::{ResourceBinding, ResourceClassId, ResourcePoolId};
use conduit_std_host::hosted_vector_index::{
    hosted_query_work, HostedHnswCorpus, HostedHnswProfile, HostedHnswProviderIdentity,
    HostedHnswRecord, HostedHnswRefusal, HostedHnswVectorIndex, HostedVectorSearchProofClass,
    HOSTED_HNSW_ALGORITHM, HOSTED_HNSW_IMPLEMENTATION_ID, HOSTED_HNSW_LIBRARY_NAME,
    HOSTED_HNSW_LIBRARY_VERSION,
};
use std::collections::BTreeSet;

fn embedding_profile() -> EmbeddingProfile {
    EmbeddingProfile {
        identity: "embedding/hosted-hnsw-fixture".into(),
        semantic_space_identity: "space/hosted-hnsw-fixture".into(),
        model_identity: "model/hosted-hnsw-fixture".into(),
        provider_identity: "provider/fixture-embedding".into(),
        dimensions: 3,
        normalization: EmbeddingNormalization::None,
        compatible_metrics: CompatibleMetrics {
            cosine_similarity: true,
            dot_product_similarity: true,
            squared_euclidean_distance: true,
        },
    }
}

fn state() -> VectorIndexState {
    VectorIndexState::new(
        VectorIndexContract {
            index_identity: "index/hosted-hnsw-fixture".into(),
            generation: 7,
            embedding_profile: embedding_profile(),
            pool_id: ResourcePoolId::from("pool/hosted-hnsw-fixture"),
            class_id: ResourceClassId::from(VECTOR_INDEX_RESOURCE_CLASS),
            bounds: VectorIndexBounds {
                maximum_items: 128,
                maximum_storage_bytes: 1_000_000,
                maximum_query_work_units: 65_536,
                maximum_results: 32,
                maximum_concurrent_queries: 1,
            },
        },
        vec![
            VectorIndexAuthorization {
                authority_identity: "authority/maintain".into(),
                authority: VectorIndexAuthority {
                    query: false,
                    insert: false,
                    upsert: false,
                    delete: false,
                    maintain: true,
                },
            },
            VectorIndexAuthorization {
                authority_identity: "authority/query".into(),
                authority: VectorIndexAuthority {
                    query: true,
                    insert: false,
                    upsert: false,
                    delete: false,
                    maintain: false,
                },
            },
        ],
    )
    .unwrap()
}

fn records() -> Vec<HostedHnswRecord<String>> {
    (0..64)
        .map(|index| {
            let angle = index as f32 * std::f32::consts::TAU / 64.0;
            HostedHnswRecord {
                record: VectorRecord {
                    value: format!("value/{index:02}"),
                    embedding: Embedding {
                        profile: embedding_profile(),
                        values: vec![angle.cos(), angle.sin(), 0.25],
                    },
                    source_identity: format!("source/{index:02}"),
                    resource_identity: format!("resource/{index:02}"),
                    metadata: vec![],
                    temporal_provenance: None,
                },
                temporal_source: TemporalSource::Event,
                boundary: None,
                transition: None,
                validity: TemporalValidity::Current,
                stored_bytes: 128,
            }
        })
        .collect()
}

fn profile(metric: SimilarityMetric) -> HostedHnswProfile {
    HostedHnswProfile {
        metric,
        seed: 0x5eed_1423,
        ef_construction: 64,
        ef_search: 32,
    }
}

fn provider(process: &str) -> HostedHnswProviderIdentity {
    HostedHnswProviderIdentity::reviewed(process).unwrap()
}

fn query(metric: SimilarityMetric, top_k: u32) -> SimilarityQuery {
    SimilarityQuery {
        embedding: Embedding {
            profile: embedding_profile(),
            values: vec![0.99, 0.12, 0.25],
        },
        metric,
        top_k,
        threshold: None,
        filters: vec![],
        temporal_intent: None,
    }
}

fn admission(work_units: u32, maximum_results: u32) -> VectorIndexQueryAdmission {
    VectorIndexQueryAdmission {
        work_units,
        maximum_results,
        concurrent_queries: 1,
    }
}

fn binding(work_units: u32) -> ResourceBinding {
    ResourceBinding {
        content: None,
        pool_id: ResourcePoolId::from("pool/hosted-hnsw-fixture"),
        class_id: ResourceClassId::from(VECTOR_INDEX_RESOURCE_CLASS),
        units: work_units,
        protected: None,
        compute: None,
    }
}

fn build(
    metric: SimilarityMetric,
) -> (
    VectorIndexState,
    HostedHnswVectorIndex<String>,
    Vec<HostedHnswRecord<String>>,
) {
    let mut state = state();
    let records = records();
    let handle = state.handle("authority/maintain").unwrap();
    let backend = HostedHnswVectorIndex::rebuild(
        &mut state,
        &handle,
        "maintenance/build-hosted-hnsw".into(),
        provider("process/host-boot-1/vector-index-1"),
        profile(metric),
        records.clone(),
    )
    .unwrap();
    (state, backend, records)
}

#[test]
fn hosted_backend_identity_profile_and_rebuild_generation_are_exact() {
    let (state, backend, _) = build(SimilarityMetric::CosineSimilarity);
    assert_eq!(state.contract.generation, 9);
    assert_eq!(backend.generation(), 9);
    assert_eq!(state.members().len(), 64);
    assert_eq!(state.stored_bytes(), 64 * 128);
    assert_eq!(
        backend.provider().implementation_identity,
        HOSTED_HNSW_IMPLEMENTATION_ID
    );
    assert_eq!(backend.provider().library_name, HOSTED_HNSW_LIBRARY_NAME);
    assert_eq!(
        backend.provider().library_version,
        HOSTED_HNSW_LIBRARY_VERSION
    );
    assert_eq!(backend.provider().algorithm, HOSTED_HNSW_ALGORITHM);
    assert_eq!(backend.profile().seed, 0x5eed_1423);
}

#[test]
fn real_hnsw_is_approximate_and_compares_honestly_with_exact_oracle() {
    for metric in [
        SimilarityMetric::CosineSimilarity,
        SimilarityMetric::DotProductSimilarity,
        SimilarityMetric::SquaredEuclideanDistance,
    ] {
        let (state, mut backend, records) = build(metric);
        let query = query(metric, 8);
        let query_handle = state.handle("authority/query").unwrap();
        let approximate_work = hosted_query_work(records.len(), 3).unwrap();
        let approximate = backend
            .query(
                &state,
                &query_handle,
                &query,
                admission(approximate_work, 8),
                &binding(approximate_work),
            )
            .unwrap();
        assert_eq!(
            approximate.proof_class,
            HostedVectorSearchProofClass::ApproximateHnsw
        );
        assert_eq!(approximate.index_generation, 9);
        assert!(approximate.approximate_candidate_count <= 32);

        let exact_candidates = records
            .into_iter()
            .map(|entry| ExactVectorSearchCandidate {
                record: entry.record,
                temporal_source: TemporalSource::Event,
                boundary: None,
                transition: None,
                validity: TemporalValidity::UnknownWhetherCurrent,
            })
            .collect::<Vec<_>>();
        let exact_work = u32::try_from(exact_candidates.len()).unwrap() * 3;
        let exact = exact_vector_search(
            &state,
            &query_handle,
            &query,
            &exact_candidates,
            admission(exact_work, 8),
            &binding(exact_work),
            true,
        )
        .unwrap();
        let exact_sources: BTreeSet<_> = exact
            .hits
            .iter()
            .map(|hit| hit.source_identity.as_str())
            .collect();
        let recalled = approximate
            .hits
            .iter()
            .filter(|hit| exact_sources.contains(hit.source_identity.as_str()))
            .count();
        assert!(recalled >= 6, "{metric:?} recalled {recalled}/8");
        assert_ne!(
            format!("{:?}", approximate.proof_class),
            format!("{:?}", exact.proof_class)
        );
    }
}

#[test]
fn fixed_seed_produces_repeatable_fixture_candidates_without_claiming_exactness() {
    let (state_a, mut backend_a, _) = build(SimilarityMetric::SquaredEuclideanDistance);
    let (state_b, mut backend_b, _) = build(SimilarityMetric::SquaredEuclideanDistance);
    let query = query(SimilarityMetric::SquaredEuclideanDistance, 12);
    let work = hosted_query_work(64, 3).unwrap();
    let first = backend_a
        .query(
            &state_a,
            &state_a.handle("authority/query").unwrap(),
            &query,
            admission(work, 12),
            &binding(work),
        )
        .unwrap();
    let second = backend_b
        .query(
            &state_b,
            &state_b.handle("authority/query").unwrap(),
            &query,
            admission(work, 12),
            &binding(work),
        )
        .unwrap();
    assert_eq!(first.hits, second.hits);
}

#[test]
fn provider_loss_stale_generation_and_hidden_mutation_refuse_distinctly() {
    let (mut state, mut backend, _) = build(SimilarityMetric::CosineSimilarity);
    assert_eq!(
        backend.refuse_hidden_mutation(),
        Err(HostedHnswRefusal::ExplicitRebuildRequired)
    );
    assert_eq!(state.contract.generation, 9);

    let old = state.handle("authority/query").unwrap();
    assert_eq!(backend.mark_provider_lost(&mut state, &old), Ok(10));
    let current = state.handle("authority/query").unwrap();
    let work = hosted_query_work(64, 3).unwrap();
    assert_eq!(
        backend.query(
            &state,
            &current,
            &query(SimilarityMetric::CosineSimilarity, 8),
            admission(work, 8),
            &binding(work),
        ),
        Err(HostedHnswRefusal::ProviderLost)
    );

    let (mut state, mut backend, _) = build(SimilarityMetric::CosineSimilarity);
    let current = state.handle("authority/query").unwrap();
    state.mark_unavailable(&current).unwrap();
    let current = state.handle("authority/query").unwrap();
    assert_eq!(
        backend.query(
            &state,
            &current,
            &query(SimilarityMetric::CosineSimilarity, 8),
            admission(work, 8),
            &binding(work),
        ),
        Err(HostedHnswRefusal::StaleBackendGeneration)
    );
}

#[test]
fn work_authority_binding_and_empty_portable_filter_are_exact() {
    let (state, mut backend, _) = build(SimilarityMetric::DotProductSimilarity);
    let query_handle = state.handle("authority/query").unwrap();
    let required = hosted_query_work(64, 3).unwrap();
    assert_eq!(
        backend.query(
            &state,
            &query_handle,
            &query(SimilarityMetric::DotProductSimilarity, 8),
            admission(required - 1, 8),
            &binding(required - 1),
        ),
        Err(HostedHnswRefusal::QueryWorkLimitExceeded)
    );
    let wrong_binding = ResourceBinding {
        content: None,
        pool_id: ResourcePoolId::from("pool/wrong"),
        ..binding(required)
    };
    assert_eq!(
        backend.query(
            &state,
            &query_handle,
            &query(SimilarityMetric::DotProductSimilarity, 8),
            admission(required, 8),
            &wrong_binding,
        ),
        Err(HostedHnswRefusal::Resource(
            VectorIndexResourceRefusal::InvalidResourceBinding
        ))
    );

    let mut filtered = query(SimilarityMetric::DotProductSimilarity, 8);
    filtered.filters.push(MetadataFilter::Present {
        key: "language".into(),
    });
    assert!(backend
        .query(
            &state,
            &query_handle,
            &filtered,
            admission(required, 8),
            &binding(required),
        )
        .unwrap()
        .hits
        .is_empty());
}

#[test]
fn portable_metadata_and_latest_temporal_intent_filter_real_hnsw_candidates() {
    let mut state = state();
    let mut records = records();
    for (index, entry) in records.iter_mut().enumerate() {
        if index == 0 {
            entry.record.metadata.push(VectorMetadata {
                key: "language".into(),
                value: "en".into(),
            });
        }
        entry.record.temporal_provenance = Some(TemporalProvenance {
            event_at: Some(index as u64),
            valid_from: None,
            valid_until: None,
            observed_at: Some(index as u64),
            recorded_at: Some(index as u64),
            ingested_at: Some(index as u64),
            retrieved_at: 100,
            reference_at: 100,
            clock_basis: ClockBasis::UnixEpochMilliseconds,
            uncertainty_millis: None,
        });
    }
    let maintenance = state.handle("authority/maintain").unwrap();
    let mut backend = HostedHnswVectorIndex::rebuild_with_history(
        &mut state,
        &maintenance,
        "maintenance/filter-temporal".into(),
        provider("process/filter-temporal"),
        profile(SimilarityMetric::CosineSimilarity),
        HostedHnswCorpus {
            records,
            earliest_history_complete: true,
        },
    )
    .unwrap();
    let handle = state.handle("authority/query").unwrap();
    let work = hosted_query_work(64, 3).unwrap();

    let mut filtered = query(SimilarityMetric::CosineSimilarity, 8);
    filtered.filters.push(MetadataFilter::Equal {
        key: "language".into(),
        value: "en".into(),
    });
    let filtered = backend
        .query(
            &state,
            &handle,
            &filtered,
            admission(work, 8),
            &binding(work),
        )
        .unwrap();
    assert_eq!(filtered.hits.len(), 1);
    assert_eq!(filtered.hits[0].source_identity, "source/00");

    let mut latest = query(SimilarityMetric::CosineSimilarity, 8);
    latest.temporal_intent = Some(TemporalRetrievalIntent::LatestEvidence);
    let latest = backend
        .query(&state, &handle, &latest, admission(work, 8), &binding(work))
        .unwrap();
    assert_eq!(latest.hits.len(), 1);
    assert_eq!(latest.hits[0].source_identity, "source/63");
}

#[test]
fn unauthorized_rebuild_and_tampered_provider_identity_never_become_current() {
    let mut state = state();
    let query_only = state.handle("authority/query").unwrap();
    assert!(matches!(
        HostedHnswVectorIndex::rebuild(
            &mut state,
            &query_only,
            "maintenance/unauthorized".into(),
            provider("process/unauthorized"),
            profile(SimilarityMetric::CosineSimilarity),
            records(),
        ),
        Err(HostedHnswRefusal::Resource(
            VectorIndexResourceRefusal::MaintenanceNotAuthorized
        ))
    ));
    assert_eq!(state.contract.generation, 7);

    let mut identity = provider("process/tampered");
    identity.library_version = "future-or-unknown".into();
    assert_eq!(
        identity.validate(),
        Err(HostedHnswRefusal::InvalidProviderIdentity)
    );
}

#[test]
fn construction_bounds_and_zero_cosine_vectors_refuse_before_lifecycle_change() {
    let mut state = state();
    let maintenance = state.handle("authority/maintain").unwrap();
    let mut invalid_profile = profile(SimilarityMetric::CosineSimilarity);
    invalid_profile.ef_search = 0;
    assert!(matches!(
        HostedHnswVectorIndex::rebuild(
            &mut state,
            &maintenance,
            "maintenance/invalid-profile".into(),
            provider("process/invalid-profile"),
            invalid_profile,
            records(),
        ),
        Err(HostedHnswRefusal::InvalidProfile)
    ));
    assert_eq!(state.contract.generation, 7);

    let mut duplicate = records();
    duplicate[1].record.source_identity = duplicate[0].record.source_identity.clone();
    assert!(matches!(
        HostedHnswVectorIndex::rebuild(
            &mut state,
            &maintenance,
            "maintenance/duplicate".into(),
            provider("process/duplicate"),
            profile(SimilarityMetric::CosineSimilarity),
            duplicate,
        ),
        Err(HostedHnswRefusal::DuplicateSource)
    ));

    let mut zero = records();
    zero[0].record.embedding.values = vec![0.0, 0.0, 0.0];
    assert!(matches!(
        HostedHnswVectorIndex::rebuild(
            &mut state,
            &maintenance,
            "maintenance/zero".into(),
            provider("process/zero"),
            profile(SimilarityMetric::CosineSimilarity),
            zero,
        ),
        Err(HostedHnswRefusal::Vector(
            conduit_ai::VectorRefusal::ZeroVector
        ))
    ));

    let mut zero_storage = records();
    zero_storage[0].stored_bytes = 0;
    assert!(matches!(
        HostedHnswVectorIndex::rebuild(
            &mut state,
            &maintenance,
            "maintenance/zero-storage".into(),
            provider("process/zero-storage"),
            profile(SimilarityMetric::CosineSimilarity),
            zero_storage,
        ),
        Err(HostedHnswRefusal::Resource(
            VectorIndexResourceRefusal::StorageLimitExceeded
        ))
    ));
    assert_eq!(state.contract.generation, 7);
}
