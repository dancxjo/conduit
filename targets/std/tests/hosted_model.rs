use conduit_ai::*;
use conduit_core::{
    semantic_digest, BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::{TensorAxisRole, TensorElement};
use conduit_std_host::hosted_model::*;

const MODEL_BYTES: &[u8] = b"scale one f32 feature by two";

struct ExactStore;
impl ModelArtifactStore for ExactStore {
    fn load(&self, reference: &BoundedResourceRef) -> Result<Vec<u8>, HostedModelRefusal> {
        if reference.identity.digest() == model_content_digest(MODEL_BYTES) {
            Ok(MODEL_BYTES.to_vec())
        } else {
            Err(HostedModelRefusal::ResourceUnavailable)
        }
    }
}

struct CorruptStore;
impl ModelArtifactStore for CorruptStore {
    fn load(&self, _: &BoundedResourceRef) -> Result<Vec<u8>, HostedModelRefusal> {
        let mut bytes = MODEL_BYTES.to_vec();
        bytes[0] ^= 1;
        Ok(bytes)
    }
}

struct ReferenceAdapter {
    realization: ModelRuntimeRealization,
}
impl HostedModelAdapter for ReferenceAdapter {
    fn realization(&self) -> &ModelRuntimeRealization {
        &self.realization
    }

    fn invoke(
        &mut self,
        artifact_bytes: &[u8],
        operation: ModelOperation,
        input: &[u8],
        maximum_output_bytes: usize,
    ) -> Result<Vec<u8>, HostedModelRefusal> {
        if artifact_bytes != MODEL_BYTES || operation != ModelOperation::Infer || input.len() != 4 {
            return Err(HostedModelRefusal::AdapterRefused);
        }
        let output = (f32::from_le_bytes(input.try_into().unwrap()) * 2.0)
            .to_le_bytes()
            .to_vec();
        if output.len() > maximum_output_bytes {
            return Err(HostedModelRefusal::AdapterFailed);
        }
        Ok(output)
    }
}

fn signature() -> ModelSignature {
    let value = || {
        ModelValueConstraint::Tensor(ModelTensorConstraint {
            elements: vec![TensorElement::F32],
            axes: vec![ModelAxisConstraint {
                role: TensorAxisRole::Feature,
                dimension: ModelDimensionConstraint::Fixed(1),
            }],
            maximum_bytes: 4,
        })
    };
    ModelSignature {
        identity: "scientific/scalar-transform@1".into(),
        compatibility_version: 1,
        operations: vec![ModelOperation::Infer],
        inputs: vec![ModelPortConstraint {
            identity: "x".into(),
            semantic_kind: "data/tensor@1".into(),
            presence: ModelPortPresence::Required,
            value: value(),
        }],
        outputs: vec![ModelPortConstraint {
            identity: "y".into(),
            semantic_kind: "data/tensor@1".into(),
            presence: ModelPortPresence::Required,
            value: value(),
        }],
    }
}

fn resource(identity: [u8; 32]) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(identity),
        content_profile: KindId::from("model/artifact/reference-matrix@1"),
        access_class: ResourceClassId::from("std/model-store/read@1"),
        extent: ResourceExtent {
            bytes: MODEL_BYTES.len() as u64,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([3; 32]),
            expires_at: None,
        },
    }
}

fn fixture() -> (ModelSignature, ModelArtifact, ReferenceAdapter) {
    let signature = signature();
    let artifact = ModelArtifact {
        architecture_profile: "scientific/scalar-linear@1".into(),
        format_profile: "model/artifact/reference-matrix@1".into(),
        precision_profile: "number/ieee754-f32-le".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: resource(model_content_digest(MODEL_BYTES)),
    };
    let adapter = ReferenceAdapter {
        realization: ModelRuntimeRealization {
            implementation_identity: "std/reference-scientific-model@1".into(),
            runtime_name: "reference-matrix".into(),
            runtime_version: "1".into(),
            runtime_build_identity: "test-build/2137".into(),
            device_profile: "cpu".into(),
            supported_formats: vec![artifact.format_profile.clone()],
            supported_precisions: vec![artifact.precision_profile.clone()],
            loaded_artifact_identity: artifact.content_identity(),
        },
    };
    (signature, artifact, adapter)
}

#[test]
fn std_host_loads_and_invokes_one_non_llm_artifact_with_separate_evidence() {
    let (signature, artifact, mut adapter) = fixture();
    let input = 1.5_f32.to_le_bytes();
    let input_identity = semantic_digest("test/scalar-input@1", &input);
    let result = invoke_hosted_model(
        &ExactStore,
        &mut adapter,
        &artifact,
        &signature,
        ModelOperation::Infer,
        input_identity,
        &input,
        1,
        4,
    )
    .unwrap();
    assert_eq!(f32::from_le_bytes(result.output.try_into().unwrap()), 3.0);
    assert_eq!(
        result.evidence.artifact_identity,
        artifact.content_identity()
    );
    assert_eq!(
        result.evidence.signature_identity,
        artifact.signature_identity
    );
    result.evidence.validate().unwrap();
    assert_eq!(
        result.evidence.runtime_implementation_identity,
        "std/reference-scientific-model@1"
    );
}

#[test]
fn changed_artifact_bytes_and_runtime_identity_refuse_before_invocation() {
    let (signature, mut artifact, mut adapter) = fixture();
    artifact.content.identity = ResourceSemanticIdentity::from_digest([9; 32]);
    adapter.realization.loaded_artifact_identity = [9; 32];
    assert_eq!(
        invoke_hosted_model(
            &ExactStore,
            &mut adapter,
            &artifact,
            &signature,
            ModelOperation::Infer,
            [4; 32],
            &1_f32.to_le_bytes(),
            1,
            4,
        ),
        Err(HostedModelRefusal::ResourceUnavailable)
    );
}

#[test]
fn corrupt_bytes_refuse_even_when_extent_and_reference_are_well_formed() {
    let (signature, artifact, mut adapter) = fixture();
    assert_eq!(
        invoke_hosted_model(
            &CorruptStore,
            &mut adapter,
            &artifact,
            &signature,
            ModelOperation::Infer,
            [4; 32],
            &1_f32.to_le_bytes(),
            1,
            4,
        ),
        Err(HostedModelRefusal::ResourceContentMismatch)
    );
}
