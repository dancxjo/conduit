use conduit_ai::*;
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::{TensorAxisRole, TensorElement};

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
    ModelTensorConstraint {
        elements: vec![TensorElement::F32],
        axes: vec![
            ModelAxisConstraint {
                role: TensorAxisRole::Time,
                dimension: ModelDimensionConstraint::Bounded {
                    minimum: 1,
                    maximum: 256,
                },
            },
            ModelAxisConstraint {
                role: TensorAxisRole::Feature,
                dimension: ModelDimensionConstraint::Fixed(12),
            },
        ],
        maximum_bytes: 12_288,
    }
}

fn signature_fixture() -> ModelSignature {
    ModelSignature {
        identity: "tongues/articulatory-encoder@1".into(),
        compatibility_version: 1,
        operations: vec![ModelOperation::Encode, ModelOperation::Evaluate],
        inputs: vec![ModelPortConstraint {
            identity: "trajectory".into(),
            semantic_kind: "data/sampled-signal@1".into(),
            presence: ModelPortPresence::Required,
            value: ModelValueConstraint::SampledSignal(tensor()),
        }],
        outputs: vec![ModelPortConstraint {
            identity: "latent".into(),
            semantic_kind: "data/tensor@1".into(),
            presence: ModelPortPresence::Required,
            value: ModelValueConstraint::Tensor(tensor()),
        }],
    }
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

    let mut bad_signal = signature_fixture();
    let ModelValueConstraint::SampledSignal(input) = &mut bad_signal.inputs[0].value else {
        unreachable!()
    };
    input.axes[0].role = TensorAxisRole::Feature;
    assert_eq!(
        bad_signal.validate(),
        Err(ModelSignatureRefusal::InvalidSignalConstraint)
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
