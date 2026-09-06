use conduit_ai::{
    install_vector_search_catalog, ClockBasis, CompatibleMetrics, Embedding,
    EmbeddingNormalization, EmbeddingProfile, SimilarityMetric, SimilarityQuery,
    TemporalProvenance, TemporalSource, TemporalValidity, VectorIndexAuthority,
    VectorIndexAuthorization, VectorIndexBounds, VectorIndexContract, VectorIndexQueryAdmission,
    VectorIndexResourceRefusal, VectorIndexState, VectorMetadata, VectorRecord,
    VECTOR_INDEX_RESOURCE_CLASS,
};
use conduit_core::{
    prepare_plan_on_hosts, ActivePlayId, BaseImplementationId, BootId, HostAdvertisement, HostId,
    HostPreparationRefusal, HostProfileId, OfferGeneration, PlanFragment, PlanPreparationError,
    PlanPreparationHost, PreparationHostIdentity, PreparedFragmentReceipt, ResourceBinding,
    ResourceClassId, ResourcePoolId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::hosted_vector_index::{
    hosted_hnsw_vector_search_offer, hosted_query_work, HostedHnswCorpus, HostedHnswProfile,
    HostedHnswProviderIdentity, HostedHnswRecord, HostedHnswRefusal, HostedHnswVectorIndex,
};

const SOURCE: &str = "form retrieval {\n search: retrieval/vector-search(4096, 8192, 1024, 8)\n}\n";
const HOST: &str = "host/vector-capstone";
const BOOT: &str = "boot/vector-capstone/1";

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProjectResource {
    RepositoryIssue { number: u32 },
    StatusSign { stage: &'static str },
    CalendarEvent { starts_at: u64 },
    CatalogRecord { kind: &'static str },
}

fn embedding_profile(revision: &str) -> EmbeddingProfile {
    EmbeddingProfile {
        identity: format!("embedding/project-history/{revision}"),
        semantic_space_identity: format!("space/project-history/{revision}"),
        model_identity: format!("model/project-history/{revision}"),
        provider_identity: "provider/reviewed-fixture".into(),
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
            index_identity: "index/project-history".into(),
            generation: 7,
            embedding_profile: embedding_profile("v1"),
            pool_id: ResourcePoolId::from("pool/project-history"),
            class_id: ResourceClassId::from(VECTOR_INDEX_RESOURCE_CLASS),
            bounds: VectorIndexBounds {
                maximum_items: 8,
                maximum_storage_bytes: 8_192,
                maximum_query_work_units: 65_536,
                maximum_results: 8,
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

fn temporal(event_at: u64) -> TemporalProvenance {
    TemporalProvenance {
        event_at: Some(event_at),
        valid_from: Some(event_at),
        valid_until: None,
        observed_at: Some(event_at + 1),
        recorded_at: Some(event_at + 2),
        ingested_at: Some(event_at + 3),
        retrieved_at: 990,
        reference_at: 1_000,
        clock_basis: ClockBasis::UnixEpochMilliseconds,
        uncertainty_millis: None,
    }
}

fn records(profile: &EmbeddingProfile) -> Vec<HostedHnswRecord<ProjectResource>> {
    [
        (
            ProjectResource::RepositoryIssue { number: 1420 },
            "source/issue/1420",
            "resource/github/issue/1420",
            "repository",
            [1.0, 0.0, 0.0],
            100,
        ),
        (
            ProjectResource::StatusSign { stage: "V5" },
            "source/sign/vector-stage",
            "resource/sign/vector-stage",
            "sign",
            [0.9, 0.1, 0.0],
            800,
        ),
        (
            ProjectResource::CalendarEvent { starts_at: 900 },
            "source/calendar/vector-demo",
            "resource/calendar/vector-demo",
            "calendar",
            [0.8, 0.2, 0.0],
            900,
        ),
        (
            ProjectResource::CatalogRecord {
                kind: "robotics/pose",
            },
            "source/catalog/robotics-pose",
            "resource/catalog/robotics-pose",
            "catalog",
            [0.7, 0.3, 0.0],
            700,
        ),
    ]
    .into_iter()
    .map(
        |(value, source, resource, family, values, event_at)| HostedHnswRecord {
            record: VectorRecord {
                value,
                embedding: Embedding {
                    profile: profile.clone(),
                    values: values.into(),
                },
                source_identity: source.into(),
                resource_identity: resource.into(),
                metadata: vec![VectorMetadata {
                    key: "family".into(),
                    value: family.into(),
                }],
                temporal_provenance: Some(temporal(event_at)),
            },
            temporal_source: TemporalSource::Event,
            boundary: None,
            transition: None,
            validity: TemporalValidity::Current,
            stored_bytes: 256,
        },
    )
    .collect()
}

fn backend_profile() -> HostedHnswProfile {
    HostedHnswProfile {
        metric: SimilarityMetric::CosineSimilarity,
        seed: 1420,
        ef_construction: 32,
        ef_search: 8,
    }
}

fn build(
    state: &mut VectorIndexState,
    profile: &EmbeddingProfile,
    operation: &str,
    process: &str,
) -> HostedHnswVectorIndex<ProjectResource> {
    let handle = state.handle("authority/maintain").unwrap();
    HostedHnswVectorIndex::rebuild_with_history(
        state,
        &handle,
        operation.into(),
        HostedHnswProviderIdentity::reviewed(process).unwrap(),
        backend_profile(),
        HostedHnswCorpus {
            records: records(profile),
            earliest_history_complete: true,
        },
    )
    .unwrap()
}

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_vector_search_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    expand_canonical_form(&checked, "retrieval", &profile).unwrap()
}

fn plan(
    expanded: &conduit_form::ExpandedCanonicalForm,
    state: &VectorIndexState,
    backend: &HostedHnswVectorIndex<ProjectResource>,
) -> conduit_core::Plan {
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(HOST),
        boot_id: BootId::from(BOOT),
        offer_generation: OfferGeneration(state.contract.generation),
        profile: HostProfileId::from("host/vector-capstone@1"),
        resources: vec![state.contract.planning_offer().unwrap()],
        capabilities: vec![
            hosted_hnsw_vector_search_offer(backend.provider(), backend.profile()).unwrap(),
        ],
        planner_capabilities: vec![],
    };
    let placements =
        conduit_planner::default_expanded_placements(expanded, core::slice::from_ref(&host))
            .unwrap();
    conduit_planner::plan_expanded_canonical(
        expanded,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap()
}

struct CurrentHost {
    identity: PreparationHostIdentity,
    prepared: Option<PreparedFragmentReceipt>,
}

impl PlanPreparationHost for CurrentHost {
    fn preparation_identity(&self) -> PreparationHostIdentity {
        self.identity.clone()
    }

    fn prepare_fragment(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<PreparedFragmentReceipt, HostPreparationRefusal> {
        let receipt = PreparedFragmentReceipt::new(fragment);
        self.prepared = Some(receipt.clone());
        Ok(receipt)
    }

    fn release_fragment(
        &mut self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        if self.prepared.as_ref() != Some(receipt) {
            return Err(HostPreparationRefusal::PreparedBindingMismatch);
        }
        self.prepared = None;
        Ok(())
    }

    fn validate_start(
        &self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        (self.prepared.as_ref() == Some(receipt))
            .then_some(())
            .ok_or(HostPreparationRefusal::PreparedBindingMismatch)
    }

    fn start_fragment(&mut self, _receipt: &PreparedFragmentReceipt) -> ActivePlayId {
        ActivePlayId::from("play/vector-capstone")
    }
}

#[test]
fn heterogeneous_resources_query_as_candidates_through_one_portable_form() {
    let mut state = state();
    let profile = embedding_profile("v1");
    let mut backend = build(&mut state, &profile, "rebuild/v1", "process/v1");
    let plan = plan(&expanded(), &state, &backend);
    assert_eq!(
        plan.fragments[0].offer_generation.0,
        state.contract.generation
    );
    assert_eq!(plan.source_document_id, expanded().source_document_id);
    assert!(!SOURCE.contains("hnsw"));

    let query = SimilarityQuery {
        embedding: Embedding {
            profile,
            values: vec![1.0, 0.0, 0.0],
        },
        metric: SimilarityMetric::CosineSimilarity,
        top_k: 4,
        threshold: None,
        filters: vec![],
        temporal_intent: None,
    };
    let work = hosted_query_work(4, 3).unwrap();
    let result = backend
        .query(
            &state,
            &state.handle("authority/query").unwrap(),
            &query,
            VectorIndexQueryAdmission {
                work_units: work,
                maximum_results: 4,
                concurrent_queries: 1,
            },
            &ResourceBinding {
                content: None,
                pool_id: state.contract.pool_id.clone(),
                class_id: state.contract.class_id.clone(),
                units: work,
                protected: None,
                compute: None,
            },
        )
        .unwrap();
    assert_eq!(result.hits.len(), 4);
    assert_eq!(result.index_generation, state.contract.generation);
    assert!(result
        .hits
        .iter()
        .all(|hit| hit.temporal_provenance.is_some()));
    assert!(result
        .hits
        .iter()
        .any(|hit| matches!(hit.value, ProjectResource::RepositoryIssue { .. })));
    assert!(result
        .hits
        .iter()
        .any(|hit| matches!(hit.value, ProjectResource::StatusSign { .. })));
    assert!(result
        .hits
        .iter()
        .any(|hit| matches!(hit.value, ProjectResource::CalendarEvent { .. })));
    assert!(result
        .hits
        .iter()
        .any(|hit| matches!(hit.value, ProjectResource::CatalogRecord { .. })));
}

#[test]
fn reembedding_requires_fresh_index_handle_offer_and_plan_truth() {
    let expanded = expanded();
    let mut state = state();
    let mut old_backend = build(
        &mut state,
        &embedding_profile("v1"),
        "rebuild/v1",
        "process/v1",
    );
    let old_handle = state.handle("authority/query").unwrap();
    let old_plan = plan(&expanded, &state, &old_backend);
    let old_plan_bytes = serde_json::to_vec(&old_plan).unwrap();
    let old_sources = state
        .members()
        .iter()
        .map(|item| item.source_identity.clone())
        .collect::<Vec<_>>();

    let fresh_profile = embedding_profile("v2");
    let fresh_backend = build(&mut state, &fresh_profile, "rebuild/v2", "process/v2");
    let fresh_plan = plan(&expanded, &state, &fresh_backend);
    assert_eq!(serde_json::to_vec(&old_plan).unwrap(), old_plan_bytes);
    assert_eq!(state.contract.embedding_profile, fresh_profile);
    assert_ne!(old_plan.plan_id, fresh_plan.plan_id);
    assert_eq!(old_plan.source_document_id, fresh_plan.source_document_id);
    assert_eq!(old_plan.checked_form_id, fresh_plan.checked_form_id);
    assert_eq!(old_plan.expanded_form_id, fresh_plan.expanded_form_id);
    assert_ne!(
        old_plan.fragments[0].offer_generation,
        fresh_plan.fragments[0].offer_generation
    );
    assert_eq!(
        state.admit_query(
            &old_handle,
            VectorIndexQueryAdmission {
                work_units: 1,
                maximum_results: 1,
                concurrent_queries: 1,
            },
            &ResourceBinding {
                content: None,
                pool_id: state.contract.pool_id.clone(),
                class_id: state.contract.class_id.clone(),
                units: 1,
                protected: None,
                compute: None,
            },
        ),
        Err(VectorIndexResourceRefusal::StaleGeneration)
    );

    let stale_query = SimilarityQuery {
        embedding: Embedding {
            profile: embedding_profile("v1"),
            values: vec![1.0, 0.0, 0.0],
        },
        metric: SimilarityMetric::CosineSimilarity,
        top_k: 1,
        threshold: None,
        filters: vec![],
        temporal_intent: None,
    };
    assert_eq!(
        old_backend.query(
            &state,
            &old_handle,
            &stale_query,
            VectorIndexQueryAdmission {
                work_units: 12,
                maximum_results: 1,
                concurrent_queries: 1,
            },
            &ResourceBinding {
                content: None,
                pool_id: state.contract.pool_id.clone(),
                class_id: state.contract.class_id.clone(),
                units: 12,
                protected: None,
                compute: None,
            },
        ),
        Err(HostedHnswRefusal::StaleBackendGeneration)
    );

    let mut current_host = CurrentHost {
        identity: PreparationHostIdentity {
            host_id: HostId::from(HOST),
            boot_id: BootId::from(BOOT),
            offer_generation: fresh_plan.fragments[0].offer_generation,
        },
        prepared: None,
    };
    assert!(matches!(
        prepare_plan_on_hosts(&old_plan, &mut [&mut current_host]),
        Err(PlanPreparationError::HostRefused {
            reason: HostPreparationRefusal::StaleOffer,
            ..
        })
    ));
    assert!(prepare_plan_on_hosts(&fresh_plan, &mut [&mut current_host]).is_ok());

    let fresh_sources = state
        .members()
        .iter()
        .map(|item| item.source_identity.clone())
        .collect::<Vec<_>>();
    assert_eq!(old_sources, fresh_sources);
}
