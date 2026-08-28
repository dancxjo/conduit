use conduit_ai::{
    CompatibleMetrics, EmbeddingNormalization, EmbeddingProfile, VectorIndexAuthority,
    VectorIndexAuthorization, VectorIndexBounds, VectorIndexContract, VectorIndexLifecycle,
    VectorIndexMaintenanceKind, VectorIndexMember, VectorIndexMutation, VectorIndexResourceRefusal,
    VectorIndexState, VECTOR_INDEX_RESOURCE_CLASS,
};
use conduit_core::{ResourceClassId, ResourcePoolId};

fn profile(identity: &str) -> EmbeddingProfile {
    EmbeddingProfile {
        identity: identity.into(),
        semantic_space_identity: format!("space/{identity}"),
        model_identity: format!("model/{identity}"),
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

fn authority(maintain: bool, insert: bool) -> VectorIndexAuthority {
    VectorIndexAuthority {
        query: true,
        insert,
        upsert: insert,
        delete: insert,
        maintain,
    }
}

fn state() -> VectorIndexState {
    VectorIndexState::new(
        VectorIndexContract {
            index_identity: "index/lifecycle".into(),
            generation: 4,
            embedding_profile: profile("profile-a"),
            pool_id: ResourcePoolId::from("pool/vector-index/lifecycle"),
            class_id: ResourceClassId::from(VECTOR_INDEX_RESOURCE_CLASS),
            bounds: VectorIndexBounds {
                maximum_items: 3,
                maximum_storage_bytes: 16,
                maximum_query_work_units: 8,
                maximum_results: 4,
                maximum_concurrent_queries: 2,
            },
        },
        vec![
            VectorIndexAuthorization {
                authority_identity: "authority/maintenance".into(),
                authority: authority(true, true),
            },
            VectorIndexAuthorization {
                authority_identity: "authority/query".into(),
                authority: authority(false, false),
            },
        ],
    )
    .unwrap()
}

fn insert(state: &mut VectorIndexState, source: &str, bytes: u64) {
    let handle = state.handle("authority/maintenance").unwrap();
    state
        .mutate(
            &handle,
            VectorIndexMutation::Insert {
                mutation_identity: format!("mutation/{source}"),
                source_identity: source.into(),
                stored_bytes: bytes,
            },
        )
        .unwrap();
}

#[test]
fn rebuild_is_two_explicit_generations_and_replaces_profile_and_membership() {
    let mut state = state();
    insert(&mut state, "source/a", 4);
    let handle = state.handle("authority/maintenance").unwrap();
    let started = state
        .begin_maintenance(
            &handle,
            "maintenance/rebuild-1".into(),
            VectorIndexMaintenanceKind::Rebuild,
        )
        .unwrap();
    assert_eq!((started.prior_generation, started.generation), (5, 6));
    assert!(!started.completed);
    assert!(matches!(
        state.lifecycle,
        VectorIndexLifecycle::Rebuilding { .. }
    ));
    assert_eq!(
        state.mutate(
            &handle,
            VectorIndexMutation::Delete {
                mutation_identity: "mutation/hidden-delete".into(),
                source_identity: "source/a".into(),
            },
        ),
        Err(VectorIndexResourceRefusal::StaleGeneration)
    );

    let active = state.handle("authority/maintenance").unwrap();
    let completed = state
        .complete_rebuild(
            &active,
            "maintenance/rebuild-1",
            profile("profile-b"),
            vec![VectorIndexMember {
                source_identity: "source/b".into(),
                stored_bytes: 6,
            }],
        )
        .unwrap();
    assert_eq!((completed.prior_generation, completed.generation), (6, 7));
    assert!(completed.completed);
    assert_eq!(state.lifecycle, VectorIndexLifecycle::Idle);
    assert_eq!(state.contract.embedding_profile.identity, "profile-b");
    assert_eq!(state.members()[0].source_identity, "source/b");
}

#[test]
fn compaction_preserves_exact_sources_while_changing_storage_truth() {
    let mut state = state();
    insert(&mut state, "source/b", 5);
    insert(&mut state, "source/a", 4);
    let handle = state.handle("authority/maintenance").unwrap();
    state
        .begin_maintenance(
            &handle,
            "maintenance/compact-1".into(),
            VectorIndexMaintenanceKind::Compaction,
        )
        .unwrap();
    let active = state.handle("authority/maintenance").unwrap();
    assert_eq!(
        state.complete_compaction(
            &active,
            "maintenance/compact-1",
            vec![VectorIndexMember {
                source_identity: "source/a".into(),
                stored_bytes: 3,
            }],
        ),
        Err(VectorIndexResourceRefusal::SourceSetMismatch)
    );
    let receipt = state
        .complete_compaction(
            &active,
            "maintenance/compact-1",
            vec![
                VectorIndexMember {
                    source_identity: "source/b".into(),
                    stored_bytes: 3,
                },
                VectorIndexMember {
                    source_identity: "source/a".into(),
                    stored_bytes: 2,
                },
            ],
        )
        .unwrap();
    assert_eq!(receipt.item_count, 2);
    assert_eq!(receipt.stored_bytes, 5);
    assert_eq!(state.contract.embedding_profile.identity, "profile-a");
    assert_eq!(state.members()[0].source_identity, "source/a");
}

#[test]
fn maintenance_authority_busy_state_and_operation_identity_fail_closed() {
    let mut state = state();
    let query = state.handle("authority/query").unwrap();
    assert_eq!(
        state.begin_maintenance(
            &query,
            "maintenance/refused".into(),
            VectorIndexMaintenanceKind::Rebuild,
        ),
        Err(VectorIndexResourceRefusal::MaintenanceNotAuthorized)
    );
    let maintenance = state.handle("authority/maintenance").unwrap();
    state
        .begin_maintenance(
            &maintenance,
            "maintenance/rebuild".into(),
            VectorIndexMaintenanceKind::Rebuild,
        )
        .unwrap();
    let active = state.handle("authority/maintenance").unwrap();
    assert_eq!(
        state.begin_maintenance(
            &active,
            "maintenance/hidden-second".into(),
            VectorIndexMaintenanceKind::Compaction,
        ),
        Err(VectorIndexResourceRefusal::ResourceBusy)
    );
    assert_eq!(
        state.complete_rebuild(&active, "maintenance/wrong", profile("profile-b"), vec![],),
        Err(VectorIndexResourceRefusal::WrongMaintenanceOperation)
    );
}

#[test]
fn cancellation_is_versioned_and_preserves_profile_and_members() {
    let mut state = state();
    insert(&mut state, "source/a", 4);
    let original_profile = state.contract.embedding_profile.clone();
    let handle = state.handle("authority/maintenance").unwrap();
    state
        .begin_maintenance(
            &handle,
            "maintenance/cancelled".into(),
            VectorIndexMaintenanceKind::Rebuild,
        )
        .unwrap();
    let active = state.handle("authority/maintenance").unwrap();
    let receipt = state
        .cancel_maintenance(&active, "maintenance/cancelled")
        .unwrap();
    assert!(receipt.cancelled);
    assert!(!receipt.completed);
    assert_eq!(state.lifecycle, VectorIndexLifecycle::Idle);
    assert_eq!(state.contract.embedding_profile, original_profile);
    assert_eq!(state.members()[0].source_identity, "source/a");
}

#[test]
fn replacement_membership_bounds_refuse_without_finishing_operation() {
    let mut state = state();
    let handle = state.handle("authority/maintenance").unwrap();
    state
        .begin_maintenance(
            &handle,
            "maintenance/oversize".into(),
            VectorIndexMaintenanceKind::Rebuild,
        )
        .unwrap();
    let active = state.handle("authority/maintenance").unwrap();
    assert_eq!(
        state.complete_rebuild(
            &active,
            "maintenance/oversize",
            profile("profile-b"),
            vec![VectorIndexMember {
                source_identity: "source/a".into(),
                stored_bytes: 17,
            }],
        ),
        Err(VectorIndexResourceRefusal::StorageLimitExceeded)
    );
    assert!(matches!(
        state.lifecycle,
        VectorIndexLifecycle::Rebuilding { .. }
    ));
    assert_eq!(state.contract.embedding_profile.identity, "profile-a");
}
