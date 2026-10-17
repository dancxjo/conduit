use conduit_ai::*;
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::{TensorAxisRole, TensorElement};

#[cfg(target_has_atomic = "ptr")]
fn model_access(artifact: &ModelArtifact) -> conduit_core::ResourceReferenceBinding {
    use conduit_core::*;
    let reference = &artifact.content;
    ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from("test/model-content"),
        authority_contract: AuthorityContractId::from(MODEL_READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("test/model-read-grant"),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
        availability: ResourceReferenceAvailability::Available,
    }
}

#[cfg(target_has_atomic = "ptr")]
#[test]
fn immutable_model_custody_retains_exact_artifact_signature_and_one_storage() {
    use std::sync::Arc;
    let signature = signature_fixture();
    let artifact = artifact_fixture(&signature);
    let bytes: Arc<[u8]> = Arc::from(&b"finite non-llm articulatory encoder"[..]);
    let pointer = bytes.as_ptr();
    let admitted = AdmittedModelResource::adopt(
        artifact.clone(),
        signature.clone(),
        bytes.clone(),
        &model_access(&artifact),
    )
    .unwrap();
    drop(bytes);
    assert_eq!(admitted.artifact(), &artifact);
    assert_eq!(admitted.signature(), &signature);
    assert_eq!(admitted.bytes().as_ptr(), pointer);
    let shared = admitted.shared_storage();
    assert_eq!(shared.as_ptr(), pointer);
    assert_eq!(admitted.access().maximum_bytes, shared.len() as u64);
    assert_eq!(
        admitted.descriptor_identity(),
        artifact.descriptor_digest(&signature).unwrap()
    );
    drop(admitted);
    assert_eq!(&*shared, b"finite non-llm articulatory encoder");
}

#[cfg(target_has_atomic = "ptr")]
#[test]
fn model_custody_refuses_changed_content_signature_extent_and_access() {
    use conduit_core::*;
    use std::sync::Arc;
    let signature = signature_fixture();
    let artifact = artifact_fixture(&signature);
    let bytes: Arc<[u8]> = Arc::from(&b"finite non-llm articulatory encoder"[..]);
    let grant = model_access(&artifact);
    let mut changed = bytes.to_vec();
    changed[0] ^= 1;
    assert!(matches!(
        AdmittedModelResource::adopt(
            artifact.clone(),
            signature.clone(),
            Arc::from(changed),
            &grant
        ),
        Err(ModelResourceRefusal::Content)
    ));
    let mut wrong_signature = signature.clone();
    wrong_signature.compatibility_version += 1;
    assert!(matches!(
        AdmittedModelResource::adopt(artifact.clone(), wrong_signature, bytes.clone(), &grant),
        Err(ModelResourceRefusal::Compatibility(
            ModelCompatibilityRefusal::SignatureMismatch
        ))
    ));
    let mut wrong_extent = artifact.clone();
    wrong_extent.content.extent.bytes += 1;
    assert!(matches!(
        AdmittedModelResource::adopt(wrong_extent, signature.clone(), bytes.clone(), &grant),
        Err(ModelResourceRefusal::Content)
    ));
    let mut wrong_grants = Vec::new();
    let mut lost = grant.clone();
    lost.availability = ResourceReferenceAvailability::Lost;
    wrong_grants.push(lost);
    let mut stale = grant.clone();
    stale.version = ResourceVersionIdentity::from_digest([8; 32]);
    wrong_grants.push(stale);
    let mut foreign = grant.clone();
    foreign.authority_contract = AuthorityContractId::from("test/foreign-authority");
    wrong_grants.push(foreign);
    let mut short = grant;
    short.maximum_bytes -= 1;
    wrong_grants.push(short);
    for invalid in wrong_grants {
        assert!(matches!(
            AdmittedModelResource::adopt(
                artifact.clone(),
                signature.clone(),
                bytes.clone(),
                &invalid
            ),
            Err(ModelResourceRefusal::Authority(_))
        ));
    }
}

fn reference(digest: [u8; 32], profile: &str, bytes: u64) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(digest),
        content_profile: KindId::from(profile),
        access_class: ResourceClassId::from("model-store/read@1"),
        extent: ResourceExtent { bytes, items: None },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([7; 32]),
            expires_at: None,
        },
    }
}

fn tensor() -> ModelTensorConstraint {
    tensor_with(
        TensorAxisRole::Time,
        ModelDimensionConstraint::bounded(256, 1).unwrap(),
    )
}

fn tensor_with(
    first_role: TensorAxisRole,
    first_dimension: ModelDimensionConstraint,
) -> ModelTensorConstraint {
    ModelTensorConstraint::from_parts(
        vec![TensorElement::F32],
        vec![
            ModelAxisConstraint {
                role: first_role,
                dimension: first_dimension,
            },
            ModelAxisConstraint {
                role: TensorAxisRole::Feature,
                dimension: ModelDimensionConstraint::fixed(12).unwrap(),
            },
        ],
        12_288,
    )
    .unwrap()
}

fn signature_fixture() -> ModelSignature {
    signature_with(tensor())
}

fn signature_with(input: ModelTensorConstraint) -> ModelSignature {
    ModelSignature::from_parts(
        "tongues/articulatory-encoder@1".into(),
        1,
        vec![ModelOperation::Encode, ModelOperation::Evaluate],
        vec![ModelPortConstraint::from_parts(
            "trajectory".into(),
            "data/sampled-signal@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::sampled_signal(input).unwrap(),
        )
        .unwrap()],
        vec![ModelPortConstraint::from_parts(
            "latent".into(),
            "data/tensor@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::tensor(tensor()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
}

fn artifact_fixture(signature: &ModelSignature) -> ModelArtifact {
    let bytes = b"finite non-llm articulatory encoder";
    ModelArtifact {
        architecture_profile: "tongues/linear-articulatory-encoder@1".into(),
        format_profile: "model/artifact/reference-matrix@1".into(),
        precision_profile: "number/ieee754-f32-le".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: reference(
            model_content_digest(bytes),
            "model/artifact/reference-matrix@1",
            bytes.len() as u64,
        ),
    }
}

#[test]
fn artifact_state_checkpoint_and_runtime_are_distinct_exact_identities() {
    let signature = signature_fixture();
    let artifact = artifact_fixture(&signature);
    artifact.validate(&signature).unwrap();
    let state = MutableModelState {
        base_artifact_identity: artifact.content_identity(),
        state_identity: "training/run-7/model-state".into(),
        state_schema_version: 1,
        generation: 9,
    };
    state.validate(&artifact).unwrap();
    let checkpoint = ModelCheckpoint {
        base_artifact_identity: artifact.content_identity(),
        architecture_profile: artifact.architecture_profile.clone(),
        state_schema_version: 1,
        generation: state.generation,
        content: reference([8; 32], MODEL_CHECKPOINT_INFO_ID, 4096),
    };
    checkpoint.validate(&artifact).unwrap();
    let runtime = ModelRuntimeRealization {
        implementation_identity: "std/reference-model-adapter@1".into(),
        runtime_name: "conduit-reference-matrix".into(),
        runtime_version: "1".into(),
        runtime_build_identity: "build/2137".into(),
        device_profile: "cpu".into(),
        supported_formats: vec![artifact.format_profile.clone()],
        supported_precisions: vec![artifact.precision_profile.clone()],
        loaded_artifact_identity: artifact.content_identity(),
        loaded_checkpoint_identity: None,
    };
    runtime.admit(&artifact).unwrap();
    assert_ne!(checkpoint.content.identity, artifact.content.identity);

    let mut incompatible_runtime = runtime;
    incompatible_runtime.supported_formats = vec!["model/artifact/other@1".into()];
    assert_eq!(
        incompatible_runtime.admit(&artifact),
        Err(ModelCompatibilityRefusal::UnsupportedFormat)
    );
}

#[test]
fn mismatched_signature_checkpoint_runtime_and_signal_shape_refuse_exactly() {
    let mut signature = signature_fixture();
    let artifact = artifact_fixture(&signature);
    signature.compatibility_version = 2;
    assert_eq!(
        artifact.validate(&signature),
        Err(ModelCompatibilityRefusal::SignatureMismatch)
    );

    let bad_signal = signature_with(tensor_with(
        TensorAxisRole::Feature,
        ModelDimensionConstraint::bounded(256, 1).unwrap(),
    ));
    assert_eq!(
        bad_signal.validate(),
        Err(ModelSignatureRefusal::InvalidSignalConstraint)
    );

    let reversed = signature_with(tensor_with(
        TensorAxisRole::Time,
        ModelDimensionConstraint::bounded(1, 2).unwrap(),
    ));
    assert_eq!(
        reversed.validate(),
        Err(ModelSignatureRefusal::InvalidTensorConstraint)
    );

    let signature = signature_fixture();
    let artifact = artifact_fixture(&signature);
    let mut checkpoint = ModelCheckpoint {
        base_artifact_identity: artifact.content_identity(),
        architecture_profile: "other/architecture@1".into(),
        state_schema_version: 1,
        generation: 1,
        content: reference([9; 32], MODEL_CHECKPOINT_INFO_ID, 1),
    };
    assert_eq!(
        checkpoint.validate(&artifact),
        Err(ModelCompatibilityRefusal::ArchitectureMismatch)
    );
    checkpoint.architecture_profile = artifact.architecture_profile.clone();
    checkpoint.state_schema_version = 2;
    assert_eq!(
        checkpoint.validate(&artifact),
        Err(ModelCompatibilityRefusal::StateSchemaMismatch)
    );
}
