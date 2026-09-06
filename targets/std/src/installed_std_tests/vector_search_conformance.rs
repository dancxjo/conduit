use super::{installed_std, RecordingTimer};
use crate::hosted_vector_index::{
    HostedHnswCorpus, HostedHnswProfile, HostedHnswProviderIdentity, HostedHnswRecord,
    HostedHnswVectorIndex,
};
use crate::hosted_vector_search::{
    ExactVectorSearchAdapter, HnswVectorSearchAdapter, HostedVectorSearchAdapter,
};
use crate::{StdHost, StdHostComposition, StdHostConfig};
use conduit_ai::{
    CompatibleMetrics, Embedding, EmbeddingNormalization, EmbeddingProfile,
    ExactVectorSearchCandidate, SimilarityMetric, TemporalSource, TemporalValidity,
    VectorIndexAuthority, VectorIndexAuthorization, VectorIndexBounds, VectorIndexContract,
    VectorIndexMutation, VectorIndexState, VectorRecord, VectorSearchExecutionProofClass,
    VectorSearchValue, VECTOR_INDEX_RESOURCE_CLASS,
};
use conduit_core::{
    BaseImplementationId, BootId, HostId, OfferGeneration, ResourceClassId, ResourcePoolId,
    TerminalDisposition,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use std::collections::BTreeMap;

const FORM: &str = "form run {\n source: conduit-test/local-model-request\n search: retrieval/vector-search(4096, 4096, 65536, 2)\n sink: conduit-test/local-model-result\n source.value > search.query\n search.hits > sink.value\n}\n";

fn profile() -> EmbeddingProfile {
    EmbeddingProfile {
        identity: "embedding/vector-play-fixture".into(),
        semantic_space_identity: "space/vector-play-fixture".into(),
        model_identity: "model/vector-play-fixture".into(),
        provider_identity: "provider/vector-play-fixture".into(),
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
            index_identity: "index/vector-play-fixture".into(),
            generation: 7,
            embedding_profile: profile(),
            pool_id: ResourcePoolId::from("pool/vector-play-fixture"),
            class_id: ResourceClassId::from(VECTOR_INDEX_RESOURCE_CLASS),
            bounds: VectorIndexBounds {
                maximum_items: 16,
                maximum_storage_bytes: 16_384,
                maximum_query_work_units: conduit_ai::MAXIMUM_VECTOR_INDEX_QUERY_WORK_UNITS,
                maximum_results: 2,
                maximum_concurrent_queries: 1,
            },
        },
        vec![
            VectorIndexAuthorization {
                authority_identity: "authority/maintain".into(),
                authority: VectorIndexAuthority {
                    query: false,
                    insert: true,
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

fn candidates() -> Vec<ExactVectorSearchCandidate<String>> {
    [
        ("alpha", [1.0, 0.0, 0.0]),
        ("beta", [0.8, 0.2, 0.0]),
        ("gamma", [0.0, 1.0, 0.0]),
    ]
    .into_iter()
    .map(|(name, values)| ExactVectorSearchCandidate {
        record: VectorRecord {
            value: format!("value/{name}"),
            embedding: Embedding {
                profile: profile(),
                values: values.into(),
            },
            source_identity: format!("source/{name}"),
            resource_identity: format!("resource/{name}"),
            metadata: Vec::new(),
            temporal_provenance: None,
        },
        temporal_source: TemporalSource::Event,
        boundary: None,
        transition: None,
        validity: TemporalValidity::Current,
    })
    .collect()
}

fn exact_adapter(process: &str) -> ExactVectorSearchAdapter {
    let mut state = state();
    for candidate in candidates() {
        let handle = state.handle("authority/maintain").unwrap();
        state
            .mutate(
                &handle,
                VectorIndexMutation::Insert {
                    mutation_identity: format!("mutation/{}", candidate.record.source_identity),
                    source_identity: candidate.record.source_identity,
                    stored_bytes: 128,
                },
            )
            .unwrap();
    }
    let handle = state.handle("authority/query").unwrap();
    ExactVectorSearchAdapter::new(process, state, handle, candidates(), true).unwrap()
}

fn hnsw_adapter(process: &str) -> HnswVectorSearchAdapter {
    let mut state = state();
    let maintain = state.handle("authority/maintain").unwrap();
    let records = candidates()
        .into_iter()
        .map(|candidate| HostedHnswRecord {
            record: candidate.record,
            temporal_source: candidate.temporal_source,
            boundary: candidate.boundary,
            transition: candidate.transition,
            validity: candidate.validity,
            stored_bytes: 128,
        })
        .collect();
    let backend = HostedHnswVectorIndex::rebuild_with_history(
        &mut state,
        &maintain,
        "maintenance/vector-play-fixture".into(),
        HostedHnswProviderIdentity::reviewed(process).unwrap(),
        HostedHnswProfile {
            metric: SimilarityMetric::CosineSimilarity,
            seed: 1423,
            ef_construction: 16,
            ef_search: 8,
        },
        HostedHnswCorpus {
            records,
            earliest_history_complete: true,
        },
    )
    .unwrap();
    let query = state.handle("authority/query").unwrap();
    HnswVectorSearchAdapter::new(state, query, backend).unwrap()
}

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_ai::install_vector_search_catalog(&mut startup, &mut profiles).unwrap();
    installed_std::test_local_model_io::install_catalog(
        &mut startup,
        &mut profiles,
        conduit_ai::SIMILARITY_QUERY_VALUE_KIND,
        conduit_ai::SIMILARITY_HITS_VALUE_KIND,
    );
    let checked = check_syntax_document(&parse_syntax_document(FORM), &startup).unwrap();
    expand_canonical_form(&checked, "run", &profiles).unwrap()
}

fn host(adapter: Box<dyn HostedVectorSearchAdapter>, suffix: &str) -> StdHost {
    let mut host = StdHost::new_with_vector_search(
        StdHostConfig {
            host_id: HostId::from(format!("host/vector-play/{suffix}")),
            boot_id: BootId::from(format!("boot/vector-play/{suffix}")),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        adapter,
    )
    .unwrap();
    host.advertisement.capabilities.extend([
        installed_std::test_local_model_io::source_offer(conduit_ai::SIMILARITY_QUERY_VALUE_KIND),
        installed_std::test_local_model_io::sink_offer(conduit_ai::SIMILARITY_HITS_VALUE_KIND),
    ]);
    // Install the fixture's complete offer set before constructing its ledger,
    // just as production constructors do for their installed capabilities.
    host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement).unwrap();
    host
}

fn plan(host: &StdHost) -> conduit_core::Plan {
    let expanded = expanded();
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4_096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
}

#[test]
fn unchanged_form_runs_through_ordinary_play_on_exact_and_hnsw() {
    let mut exact = host(Box::new(exact_adapter("process/exact-play")), "exact");
    let mut hnsw = host(Box::new(hnsw_adapter("process/hnsw-play")), "hnsw");
    let exact_plan = plan(&exact);
    let hnsw_plan = plan(&hnsw);
    assert_eq!(exact_plan.source_document_id, hnsw_plan.source_document_id);
    assert_eq!(exact_plan.checked_form_id, hnsw_plan.checked_form_id);
    assert_eq!(exact_plan.expanded_form_id, hnsw_plan.expanded_form_id);

    for (host, plan) in [(&mut exact, exact_plan), (&mut hnsw, hnsw_plan)] {
        let mut output = Vec::with_capacity(2_048);
        let mut timer = RecordingTimer { waits: Vec::new() };
        let report = host
            .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
            .expect("portable vector search executes through ordinary production Play");
        assert!(matches!(
            report.observations.last().map(|item| &item.kind),
            Some(conduit_core::ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            })
        ));
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(timer.waits.is_empty());
    }
}

#[test]
fn adapters_expose_equal_hit_semantics_but_distinct_proof_classes() {
    let exact_host = host(
        Box::new(exact_adapter("process/exact-direct")),
        "exact-direct",
    );
    let hnsw_host = host(Box::new(hnsw_adapter("process/hnsw-direct")), "hnsw-direct");
    let exact_plan = plan(&exact_host);
    let hnsw_plan = plan(&hnsw_host);
    let query = serde_json::to_vec(&conduit_ai::SimilarityQuery {
        embedding: Embedding {
            profile: profile(),
            values: vec![1.0, 0.0, 0.0],
        },
        metric: SimilarityMetric::CosineSimilarity,
        top_k: 2,
        threshold: None,
        filters: Vec::new(),
        temporal_intent: None,
    })
    .unwrap();
    let mut exact = exact_adapter("process/exact-direct");
    let mut hnsw = hnsw_adapter("process/hnsw-direct");
    let mut exact_output = Vec::new();
    let mut hnsw_output = Vec::new();
    let exact_placement = exact_plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_ai::VECTOR_SEARCH_KIND)
        .unwrap();
    let hnsw_placement = hnsw_plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_ai::VECTOR_SEARCH_KIND)
        .unwrap();
    assert_eq!(
        exact.execute(exact_placement, &query, &mut exact_output),
        crate::hosted_vector_search::HostedVectorSearchTerminal::Produced
    );
    assert_eq!(
        hnsw.execute(hnsw_placement, &query, &mut hnsw_output),
        crate::hosted_vector_search::HostedVectorSearchTerminal::Produced
    );
    let exact: VectorSearchValue<String> = serde_json::from_slice(&exact_output).unwrap();
    let hnsw: VectorSearchValue<String> = serde_json::from_slice(&hnsw_output).unwrap();
    assert_eq!(
        exact.proof_class,
        VectorSearchExecutionProofClass::DeterministicExact
    );
    assert_eq!(
        hnsw.proof_class,
        VectorSearchExecutionProofClass::Approximate
    );
    assert_eq!(
        exact
            .hits
            .iter()
            .map(|hit| hit.source_identity.as_str())
            .collect::<Vec<_>>(),
        hnsw.hits
            .iter()
            .map(|hit| hit.source_identity.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn provider_loss_is_terminal_and_emits_no_retrieval_value() {
    let planned = host(
        Box::new(hnsw_adapter("process/hnsw-provider-loss")),
        "hnsw-provider-loss",
    );
    let plan = plan(&planned);
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_ai::VECTOR_SEARCH_KIND)
        .unwrap();
    let query = serde_json::to_vec(&conduit_ai::SimilarityQuery {
        embedding: Embedding {
            profile: profile(),
            values: vec![1.0, 0.0, 0.0],
        },
        metric: SimilarityMetric::CosineSimilarity,
        top_k: 2,
        threshold: None,
        filters: Vec::new(),
        temporal_intent: None,
    })
    .unwrap();
    let mut adapter = hnsw_adapter("process/hnsw-provider-loss");
    adapter.mark_provider_lost().unwrap();
    let mut output = Vec::new();
    assert_eq!(
        adapter.execute(placement, &query, &mut output),
        crate::hosted_vector_search::HostedVectorSearchTerminal::ProviderLost
    );
    assert!(output.is_empty());
}
