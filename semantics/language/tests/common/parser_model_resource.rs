use conduit_ai::{integer_categorical::*, integer_categorical_step::PreparedCategoricalStep, *};
use conduit_core::*;
use std::sync::Arc;
pub fn categorical(
    bytes: Vec<u8>,
    signature: ModelSignature,
    state: u32,
) -> Arc<PreparedCategoricalStep> {
    let digest = model_content_digest(&bytes);
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(digest),
        content_profile: CATEGORICAL_I16_FORMAT.into(),
        access_class: "model/pinned-training-artifact/read@1".into(),
        extent: ResourceExtent {
            bytes: bytes.len() as u64,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest(digest),
            expires_at: None,
        },
    };
    let artifact = ModelArtifact {
        architecture_profile: CATEGORICAL_I16_ARCHITECTURE.into(),
        format_profile: CATEGORICAL_I16_FORMAT.into(),
        precision_profile: CATEGORICAL_I16_PRECISION.into(),
        state_schema_version: state,
        signature_identity: signature.semantic_digest().unwrap(),
        content: reference.clone(),
    };
    let binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "model/test/read".into(),
        authority_contract: MODEL_READ_AUTHORITY.into(),
        authority_grant: "test/exact-model".into(),
        maximum_bytes: reference.extent.bytes,
        maximum_items: Some(1),
        availability: ResourceReferenceAvailability::Available,
    };
    let model = Arc::new(
        AdmittedModelResource::adopt(artifact, signature, Arc::from(bytes), &binding).unwrap(),
    );
    let residence = ResourceContentOffer {
        contract: ResourceContentRequirement {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile,
            maximum_bytes: reference.extent.bytes as u32,
            maximum_items: 1,
            retention: ResourceRetention::Play,
            sharing: ResourceSharing::ImmutableReadMany,
            access: ResourceAccessMode::ReadPublished,
            generation_slots: 1,
            reader_leases: 16,
            publication_slots: 0,
            sensitive: false,
        },
        owner_host: "test/model-host".into(),
        owner_boot: "test/model-boot".into(),
        base_id: "test/model-base".into(),
        residence_profile: "test/immutable-memory".into(),
    };
    Arc::new(PreparedCategoricalStep::prepare(model, "test/model-pool".into(), residence).unwrap())
}
