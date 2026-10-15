use conduit_ai::{imported_onnx::*, *};
use conduit_core::*;
use conduit_data::{TensorAxisRole, TensorElement};
use sha2::{Digest, Sha256};
use std::sync::Arc;
#[path = "common/onnx_fixture.rs"]
mod fixture;

#[path = "common/model_signature_fixture.rs"]
mod signature_fixture;
use signature_fixture::signature;

fn resource(signature: ModelSignature) -> AdmittedModelResource {
    let bytes: Arc<[u8]> = Arc::from(fixture::model(&[]));
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

fn sidecar(signature: &ModelSignature) -> OnnxSidecar {
    let binding = |semantic: &str, graph: &str| ImportedPortBinding {
        semantic_port: semantic.into(),
        graph_name: graph.into(),
    };
    OnnxSidecar {
        graph_sha256: Sha256::digest(fixture::model(&[])).into(),
        graph_name: "tiny-mixed-input".into(),
        signature_identity: signature.semantic_digest().unwrap(),
        inputs: vec![binding("identifiers", "tokens"), binding("gain", "scale")],
        outputs: vec![binding("prediction", "result")],
    }
}

#[test]
fn binds_checkpoint_names_without_changing_semantic_ports_or_resource_custody() {
    let signature = signature();
    let resource = resource(signature.clone());
    let pointer = resource.bytes().as_ptr();
    let admitted = ImportedOnnxResource::bind(resource, &sidecar(&signature)).unwrap();
    assert_eq!(admitted.resource().bytes().as_ptr(), pointer);
    assert_eq!(admitted.resource().signature(), &signature);
    assert_eq!(admitted.metadata().inputs[0].name, "tokens");
    assert_ne!(admitted.sidecar_identity(), [0; 32]);
}

#[test]
fn wrong_graph_signature_count_names_and_digest_refuse() {
    let signature = signature();
    for (mutated, expected) in [
        (
            {
                let mut s = sidecar(&signature);
                s.graph_name = "other".into();
                s
            },
            ImportedOnnxRefusal::GraphName,
        ),
        (
            {
                let mut s = sidecar(&signature);
                s.signature_identity = [0; 32];
                s
            },
            ImportedOnnxRefusal::Signature,
        ),
        (
            {
                let mut s = sidecar(&signature);
                s.inputs.pop();
                s
            },
            ImportedOnnxRefusal::PortCount,
        ),
        (
            {
                let mut s = sidecar(&signature);
                s.inputs[0].graph_name = "missing".into();
                s
            },
            ImportedOnnxRefusal::PortName,
        ),
        (
            {
                let mut s = sidecar(&signature);
                s.inputs[0].semantic_port = "tokens".into();
                s
            },
            ImportedOnnxRefusal::PortName,
        ),
        (
            {
                let mut s = sidecar(&signature);
                s.graph_sha256 = [0; 32];
                s
            },
            ImportedOnnxRefusal::Metadata(onnx_metadata::OnnxMetadataRefusal::DigestMismatch),
        ),
    ] {
        assert_eq!(
            ImportedOnnxResource::bind(resource(signature.clone()), &mutated).err(),
            Some(expected)
        );
    }
}

#[test]
fn semantic_dtype_rank_and_dimension_must_match_the_import() {
    for (index, element, axes, expected) in [
        (
            0,
            TensorElement::F32,
            vec![ModelDimensionConstraint::bounded(32, 1).unwrap()],
            ImportedOnnxRefusal::Dtype,
        ),
        (
            0,
            TensorElement::I64,
            vec![
                ModelDimensionConstraint::fixed(1).unwrap(),
                ModelDimensionConstraint::fixed(1).unwrap(),
            ],
            ImportedOnnxRefusal::Rank,
        ),
        (
            1,
            TensorElement::F32,
            vec![ModelDimensionConstraint::fixed(2).unwrap()],
            ImportedOnnxRefusal::Dimension,
        ),
    ] {
        let original = signature();
        let mut inputs = original.inputs.get().as_slice().to_vec();
        inputs[index].value = ModelValueConstraint::tensor(
            ModelTensorConstraint::from_parts(
                vec![element],
                axes.into_iter()
                    .map(|d| ModelAxisConstraint::new(d, TensorAxisRole::Feature).unwrap())
                    .collect(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();
        let signature = ModelSignature::from_parts(
            "test/non-speech-model@1".into(),
            1,
            vec![ModelOperation::Infer],
            inputs,
            original.outputs.get().as_slice().to_vec(),
        )
        .unwrap();
        assert_eq!(
            ImportedOnnxResource::bind(resource(signature.clone()), &sidecar(&signature)).err(),
            Some(expected)
        );
    }
}
