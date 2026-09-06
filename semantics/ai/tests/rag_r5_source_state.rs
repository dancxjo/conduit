use conduit_ai::{
    extract_source, CompatibleMetrics, EmbeddingNormalization, EmbeddingProfile,
    SourceExtractionLimits, SourceExtractionProfile, SourceExtractionRefusal, SourcePayload,
    SourceRef, VectorIndexAuthority, VectorIndexAuthorization, VectorIndexBounds,
    VectorIndexContract, VectorIndexMutation, VectorIndexResourceRefusal, VectorIndexState,
    VECTOR_INDEX_RESOURCE_CLASS,
};
use conduit_core::{
    AuthorityContractId, AuthorityGrantId, BoundedResourceRef, KindId,
    ResourceDereferenceRequirement, ResourceExtent, ResourceHandleId, ResourceLifetime,
    ResourceReferenceAccessRefusal, ResourceReferenceAvailability, ResourceReferenceBinding,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};

const ACCESS_CLASS: &str = "resource/read-authorized@1";

fn source(version: u8) -> SourceRef {
    SourceRef {
        resource: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([7; 32]),
            content_profile: KindId::from("document/text-utf8@1"),
            access_class: conduit_core::ResourceClassId::from(ACCESS_CLASS),
            extent: ResourceExtent {
                bytes: 11,
                items: None,
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([version; 32]),
                expires_at: None,
            },
        },
    }
}

fn requirement(source: &SourceRef) -> ResourceDereferenceRequirement {
    ResourceDereferenceRequirement {
        content_profile: source.resource.content_profile.clone(),
        access_class: source.resource.access_class.clone(),
        authority_contract: AuthorityContractId::from(conduit_ai::SOURCE_READ_AUTHORITY),
        maximum_bytes: source.resource.extent.bytes,
        maximum_items: None,
    }
}

fn binding(source: &SourceRef) -> ResourceReferenceBinding {
    ResourceReferenceBinding {
        identity: source.resource.identity,
        version: source.resource.lifetime.version,
        content_profile: source.resource.content_profile.clone(),
        access_class: source.resource.access_class.clone(),
        handle: ResourceHandleId::from("host/source-7"),
        authority_contract: AuthorityContractId::from(conduit_ai::SOURCE_READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("grant/source-7"),
        maximum_bytes: source.resource.extent.bytes,
        maximum_items: None,
        availability: ResourceReferenceAvailability::Available,
    }
}

fn extract(
    source: &SourceRef,
    binding: &ResourceReferenceBinding,
) -> Result<conduit_ai::SourceExtractionReceipt, SourceExtractionRefusal> {
    extract_source(
        source,
        &requirement(source),
        binding,
        SourceExtractionProfile::TextUtf8 { overlap_bytes: 0 },
        SourceExtractionLimits {
            maximum_source_bytes: 32,
            maximum_source_items: 1,
            maximum_chunk_bytes: 32,
            maximum_chunks: 1,
            maximum_output_bytes: 32,
            maximum_work_units: 64,
        },
        &SourcePayload::Text(b"source text".to_vec()),
    )
}

#[test]
fn updated_deleted_and_revoked_sources_never_rebind_old_chunks() {
    let old = source(1);
    let current = source(2);
    let old_chunk = extract(&old, &binding(&old)).unwrap().chunks.remove(0);
    let current_chunk = extract(&current, &binding(&current))
        .unwrap()
        .chunks
        .remove(0);
    assert_ne!(old_chunk.identity, current_chunk.identity);
    assert_ne!(
        old_chunk.lineage.source.resource.lifetime.version,
        current_chunk.lineage.source.resource.lifetime.version
    );
    assert_eq!(
        extract(&current, &binding(&old)),
        Err(SourceExtractionRefusal::ResourceAccess(
            ResourceReferenceAccessRefusal::StaleVersion
        ))
    );
    let mut deleted = binding(&current);
    deleted.availability = ResourceReferenceAvailability::Lost;
    assert_eq!(
        extract(&current, &deleted),
        Err(SourceExtractionRefusal::ResourceAccess(
            ResourceReferenceAccessRefusal::ResourceLost
        ))
    );
    let mut revoked = binding(&current);
    revoked.authority_grant = AuthorityGrantId::from("");
    assert_eq!(
        extract(&current, &revoked),
        Err(SourceExtractionRefusal::ResourceAccess(
            ResourceReferenceAccessRefusal::EmptyAuthorityGrant
        ))
    );
}

fn index_state() -> VectorIndexState {
    VectorIndexState::new(
        VectorIndexContract {
            index_identity: "index/rag".into(),
            generation: 4,
            embedding_profile: EmbeddingProfile {
                identity: "embedding/rag@1".into(),
                semantic_space_identity: "space/rag@1".into(),
                model_identity: "model/fixture@1".into(),
                provider_identity: "provider/reviewed-fixture".into(),
                dimensions: 3,
                normalization: EmbeddingNormalization::None,
                compatible_metrics: CompatibleMetrics {
                    cosine_similarity: true,
                    dot_product_similarity: false,
                    squared_euclidean_distance: false,
                },
            },
            pool_id: conduit_core::ResourcePoolId::from("pool/rag-index"),
            class_id: conduit_core::ResourceClassId::from(VECTOR_INDEX_RESOURCE_CLASS),
            bounds: VectorIndexBounds {
                maximum_items: 4,
                maximum_storage_bytes: 64,
                maximum_query_work_units: 16,
                maximum_results: 4,
                maximum_concurrent_queries: 1,
            },
        },
        vec![VectorIndexAuthorization {
            authority_identity: "authority/index".into(),
            authority: VectorIndexAuthority {
                query: true,
                insert: true,
                upsert: true,
                delete: true,
                maintain: false,
            },
        }],
    )
    .unwrap()
}

#[test]
fn index_updates_deletions_and_loss_invalidate_every_prior_generation() {
    let mut state = index_state();
    let generation_four = state.handle("authority/index").unwrap();
    state
        .mutate(
            &generation_four,
            VectorIndexMutation::Insert {
                mutation_identity: "mutation/insert/source-v1".into(),
                source_identity: "source/version/1".into(),
                stored_bytes: 8,
            },
        )
        .unwrap();
    assert_eq!(
        state.mutate(
            &generation_four,
            VectorIndexMutation::Delete {
                mutation_identity: "mutation/stale-delete".into(),
                source_identity: "source/version/1".into(),
            },
        ),
        Err(VectorIndexResourceRefusal::StaleGeneration)
    );
    let generation_five = state.handle("authority/index").unwrap();
    let deletion = state
        .mutate(
            &generation_five,
            VectorIndexMutation::Delete {
                mutation_identity: "mutation/delete/source-v1".into(),
                source_identity: "source/version/1".into(),
            },
        )
        .unwrap();
    assert_eq!(deletion.item_count, 0);
    assert!(state.members().is_empty());
    let generation_six = state.handle("authority/index").unwrap();
    state.mark_unavailable(&generation_six).unwrap();
    assert_eq!(
        state.handle("authority/index").unwrap().generation,
        generation_six.generation + 1
    );
    assert_eq!(
        state.admit_query(
            &generation_six,
            conduit_ai::VectorIndexQueryAdmission {
                work_units: 1,
                maximum_results: 1,
                concurrent_queries: 1,
            },
            &conduit_core::ResourceBinding {
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
}
