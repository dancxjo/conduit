use conduit_ai::{imported_onnx::ONNX_FORMAT_PROFILE, *};
use conduit_core::*;
use std::sync::Arc;

pub fn resource(signature: ModelSignature, model: Vec<u8>) -> AdmittedModelResource {
    let bytes: Arc<[u8]> = Arc::from(model);
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(model_content_digest(&bytes)),
        content_profile: KindId::from(ONNX_FORMAT_PROFILE),
        access_class: ResourceClassId::from("test/immutable-model"),
        extent: ResourceExtent {
            bytes: bytes.len() as u64,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([7; 32]),
            expires_at: None,
        },
    };
    let binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from("test/local-model"),
        authority_contract: AuthorityContractId::from(MODEL_READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("test/read-grant"),
        maximum_bytes: reference.extent.bytes,
        maximum_items: None,
        availability: ResourceReferenceAvailability::Available,
    };
    let artifact = ModelArtifact {
        architecture_profile: "test/tiny-opaque@1".into(),
        format_profile: ONNX_FORMAT_PROFILE.into(),
        precision_profile: "test/i64-f32@1".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: reference,
    };
    AdmittedModelResource::adopt(artifact, signature, bytes, &binding).unwrap()
}
