use conduit_ai::{
    CompatibleMetrics, EmbeddingNormalization, EmbeddingProfile, VectorIndexAuthority,
    VectorIndexAuthorization, VectorIndexBounds, VectorIndexContract, VectorIndexMutation,
    VectorIndexQueryAdmission, VectorIndexResourceRefusal, VectorIndexState,
    VECTOR_INDEX_RESOURCE_CLASS,
};
use conduit_core::{ResourceBinding, ResourceClassId, ResourcePoolId};

fn profile() -> EmbeddingProfile {
    EmbeddingProfile {
        identity: "embedding/profile-1".into(),
        semantic_space_identity: "space/documents-v1".into(),
        model_identity: "model/fixture-v1".into(),
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
            index_identity: "index/research-notes".into(),
            generation: 7,
            embedding_profile: profile(),
            pool_id: ResourcePoolId::from("pool/vector-index/research-notes"),
            class_id: ResourceClassId::from(VECTOR_INDEX_RESOURCE_CLASS),
            bounds: VectorIndexBounds {
                maximum_items: 2,
                maximum_storage_bytes: 12,
                maximum_query_work_units: 8,
                maximum_results: 4,
                maximum_concurrent_queries: 2,
            },
        },
        vec![
            VectorIndexAuthorization {
                authority_identity: "authority/query".into(),
                authority: authority(true, false, false, false),
            },
            VectorIndexAuthorization {
                authority_identity: "authority/mutate".into(),
                authority: authority(false, true, true, true),
            },
            VectorIndexAuthorization {
                authority_identity: "authority/all".into(),
                authority: authority(true, true, true, true),
            },
        ],
    )
    .unwrap()
}

fn authority(query: bool, insert: bool, upsert: bool, delete: bool) -> VectorIndexAuthority {
    VectorIndexAuthority {
        query,
        insert,
        upsert,
        delete,
        maintain: false,
    }
}

fn insert(source: &str, bytes: u64) -> VectorIndexMutation {
    VectorIndexMutation::Insert {
        mutation_identity: format!("mutation/insert/{source}"),
        source_identity: source.into(),
        stored_bytes: bytes,
    }
}

#[test]
fn query_only_authority_is_distinct_from_every_mutation_authority() {
    let mut state = state();
    let query_only = state.handle("authority/query").unwrap();
    assert_eq!(
        state.mutate(&query_only, insert("source/a", 4)),
        Err(VectorIndexResourceRefusal::InsertNotAuthorized)
    );
    let mut forged = query_only.clone();
    forged.authority_identity = "authority/fabricated-admin".into();
    assert_eq!(
        state.mutate(&forged, insert("source/forged", 4)),
        Err(VectorIndexResourceRefusal::UnknownAuthority)
    );

    let mutation_only = state.handle("authority/mutate").unwrap();
    let binding = ResourceBinding {
        content: None,
        pool_id: state.contract.pool_id.clone(),
        class_id: state.contract.class_id.clone(),
        units: 2,
        protected: None,
        compute: None,
    };
    assert_eq!(
        state.admit_query(
            &mutation_only,
            VectorIndexQueryAdmission {
                work_units: 2,
                maximum_results: 2,
                concurrent_queries: 1,
            },
            &binding,
        ),
        Err(VectorIndexResourceRefusal::QueryNotAuthorized)
    );
}

#[test]
fn mutations_advance_one_generation_and_stale_handles_never_rebind() {
    let mut state = state();
    let handle = state.handle("authority/mutate").unwrap();
    let receipt = state.mutate(&handle, insert("sign/source-a", 4)).unwrap();
    assert_eq!(receipt.prior_generation, 7);
    assert_eq!(receipt.generation, 8);
    assert_eq!(receipt.source_identity, "sign/source-a");
    assert_eq!(receipt.item_count, 1);
    assert_eq!(receipt.stored_bytes, 4);
    assert_eq!(
        state.mutate(&handle, insert("sign/source-b", 4)),
        Err(VectorIndexResourceRefusal::StaleGeneration)
    );

    let current = state.handle("authority/mutate").unwrap();
    let upsert = state
        .mutate(
            &current,
            VectorIndexMutation::Upsert {
                mutation_identity: "mutation/upsert/a".into(),
                source_identity: "sign/source-a".into(),
                stored_bytes: 6,
            },
        )
        .unwrap();
    assert_eq!((upsert.prior_generation, upsert.generation), (8, 9));
    assert_eq!(state.members()[0].source_identity, "sign/source-a");
}

#[test]
fn item_storage_and_membership_truth_fail_closed() {
    let mut state = state();
    let mut handle = state.handle("authority/mutate").unwrap();
    state.mutate(&handle, insert("source/a", 6)).unwrap();
    handle = state.handle("authority/mutate").unwrap();
    state.mutate(&handle, insert("source/b", 6)).unwrap();
    handle = state.handle("authority/mutate").unwrap();
    assert_eq!(
        state.mutate(&handle, insert("source/c", 1)),
        Err(VectorIndexResourceRefusal::ItemLimitExceeded)
    );
    assert_eq!(
        state.mutate(
            &handle,
            VectorIndexMutation::Upsert {
                mutation_identity: "mutation/oversize".into(),
                source_identity: "source/a".into(),
                stored_bytes: 7,
            },
        ),
        Err(VectorIndexResourceRefusal::StorageLimitExceeded)
    );
    assert_eq!(
        state.mutate(
            &handle,
            VectorIndexMutation::Delete {
                mutation_identity: "mutation/delete/missing".into(),
                source_identity: "source/missing".into(),
            },
        ),
        Err(VectorIndexResourceRefusal::SourceNotPresent)
    );
}

#[test]
fn query_bounds_are_admitted_by_the_generic_resource_binding() {
    let state = state();
    let handle = state.handle("authority/query").unwrap();
    let admission = VectorIndexQueryAdmission {
        work_units: 3,
        maximum_results: 4,
        concurrent_queries: 2,
    };
    let binding = ResourceBinding {
        content: None,
        pool_id: state.contract.pool_id.clone(),
        class_id: state.contract.class_id.clone(),
        units: 3,
        protected: None,
        compute: None,
    };
    state.admit_query(&handle, admission, &binding).unwrap();

    let wrong_units = ResourceBinding {
        units: 2,
        ..binding.clone()
    };
    assert_eq!(
        state.admit_query(&handle, admission, &wrong_units),
        Err(VectorIndexResourceRefusal::InvalidResourceBinding)
    );
    assert_eq!(
        state.admit_query(
            &handle,
            VectorIndexQueryAdmission {
                work_units: 9,
                ..admission
            },
            &binding,
        ),
        Err(VectorIndexResourceRefusal::QueryWorkLimitExceeded)
    );
    assert_eq!(
        state.admit_query(
            &handle,
            VectorIndexQueryAdmission {
                maximum_results: 5,
                ..admission
            },
            &binding,
        ),
        Err(VectorIndexResourceRefusal::ResultLimitExceeded)
    );
    assert_eq!(
        state.admit_query(
            &handle,
            VectorIndexQueryAdmission {
                concurrent_queries: 3,
                ..admission
            },
            &binding,
        ),
        Err(VectorIndexResourceRefusal::QueryConcurrencyExceeded)
    );
}

#[test]
fn provider_loss_invalidates_old_handles_without_erasing_membership() {
    let mut state = state();
    let handle = state.handle("authority/all").unwrap();
    state.mutate(&handle, insert("source/a", 4)).unwrap();
    let current = state.handle("authority/all").unwrap();
    assert_eq!(state.mark_unavailable(&current), Ok(9));
    assert_eq!(state.members().len(), 1);
    let unavailable = state.handle("authority/all").unwrap();
    assert_eq!(
        state.admit_query(
            &unavailable,
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
        Err(VectorIndexResourceRefusal::ResourceUnavailable)
    );
}

#[test]
fn serialized_contract_is_portable_and_contains_no_provider_engine_choice() {
    let contract = state().contract;
    let encoded = serde_json::to_string(&contract).unwrap();
    let decoded: VectorIndexContract = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, contract);
    for forbidden in ["hnsw", "ivf", "sqlite", "postgres", "qdrant"] {
        assert!(!encoded.contains(forbidden));
    }
}

#[test]
fn exhausted_generation_refuses_before_mutating_membership() {
    let mut state = state();
    state.contract.generation = u64::MAX;
    let handle = state.handle("authority/mutate").unwrap();
    assert_eq!(
        state.mutate(&handle, insert("source/a", 4)),
        Err(VectorIndexResourceRefusal::GenerationExhausted)
    );
    assert!(state.members().is_empty());
    assert_eq!(state.stored_bytes(), 0);
}
