//! One explicitly configured installed runtime and canonical scalar model custody.
use conduit_ai::{imported_onnx::*, *};
use conduit_core::*;
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};
use conduit_std_host::native_model_backing::{NativeBackingGrant, NativeCpuProfile};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc};
pub const SOURCE: &str = include_str!("../fixtures/model-scalar-f32.conduit");
pub fn worker() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_conduit-native-model-worker"))
}
pub fn profile() -> NativeCpuProfile {
    let runtime_library = std::env::var("CONDUIT_ORT_LIBRARY")
        .expect("explicit installed runtime configuration required");
    NativeCpuProfile {
        runtime_sha256: [
            156, 143, 97, 18, 233, 220, 240, 158, 212, 230, 33, 161, 208, 167, 182, 186, 73, 87,
            92, 125, 9, 242, 122, 101, 45, 36, 162, 44, 133, 100, 177, 60,
        ],
        runtime_library,
        maximum_calls: 1024,
    }
}
pub fn grant() -> NativeBackingGrant {
    NativeBackingGrant {
        maximum_private_bytes: 64 * 1024 * 1024,
        maximum_fixed_mapping_bytes: 256 * 1024 * 1024,
    }
}
pub fn model(signature: ModelSignature) -> ImportedOnnxResource {
    let bytes: Arc<[u8]> =
        Arc::from(include_bytes!("../fixtures/model-scalar-f32.onnx").as_slice());
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(model_content_digest(&bytes)),
        content_profile: ONNX_FORMAT_PROFILE.into(),
        access_class: "proof/native-source-model".into(),
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
        handle: "proof/native-source-model".into(),
        authority_contract: MODEL_READ_AUTHORITY.into(),
        authority_grant: "proof/native-source-read".into(),
        maximum_bytes: reference.extent.bytes,
        maximum_items: None,
        availability: ResourceReferenceAvailability::Available,
    };
    let artifact = ModelArtifact {
        architecture_profile: "model/scalar-identity@1".into(),
        format_profile: ONNX_FORMAT_PROFILE.into(),
        precision_profile: "tensor/f32@1".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: reference,
    };
    let sidecar = OnnxSidecar {
        graph_sha256: Sha256::digest(&bytes).into(),
        graph_name: "bounded-scalar-identity".into(),
        signature_identity: artifact.signature_identity,
        inputs: vec![ImportedPortBinding {
            semantic_port: "scalar".into(),
            graph_name: "x".into(),
        }],
        outputs: vec![ImportedPortBinding {
            semantic_port: "scalar".into(),
            graph_name: "y".into(),
        }],
    };
    ImportedOnnxResource::bind(
        AdmittedModelResource::adopt(artifact, signature, bytes, &binding).unwrap(),
        &sidecar,
    )
    .unwrap()
}
pub fn scalar(bits: u32) -> NamedTensorBundle {
    let bytes = bits.to_le_bytes();
    NamedTensorBundle::from_values(vec![NamedTensor {
        name: "scalar".into(),
        value: TensorValue {
            element: TensorElement::F32,
            dimensions: BoundedSequence::new(),
            axes: BoundedSequence::new(),
            content_digest: tensor_content_digest(&bytes),
            backing: TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap()),
        },
    }])
    .unwrap()
}
pub const BIT_CONTROLS: [u32; 19] = [
    0x3fa00000, 0xc1000000, 0x00000000, 0x80000000, 0x7f800000, 0xff800000, 0x7fc00000, 0x7fc12345,
    0xffc54321, 0x7f800001, 0xff800001, 0x00000001, 0x80000001, 0x007fffff, 0x807fffff, 0x00800000,
    0x80800000, 0x7f7fffff, 0xff7fffff,
];

pub fn reference(bits: u32) -> NamedTensorBundle {
    let bytes = bits.to_le_bytes();
    let digest = tensor_content_digest(&bytes);
    NamedTensorBundle::from_values(vec![NamedTensor {
        name: "scalar".into(),
        value: TensorValue {
            element: TensorElement::F32,
            dimensions: BoundedSequence::new(),
            axes: BoundedSequence::new(),
            content_digest: digest,
            backing: TensorBacking::Resource(BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest(digest),
                content_profile: "tensor/elements-ieee754-f32-le@1".into(),
                access_class: "proof/ungranted-scalar-storage".into(),
                extent: ResourceExtent {
                    bytes: 4,
                    items: Some(1),
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest([9; 32]),
                    expires_at: None,
                },
            }),
        },
    }])
    .unwrap()
}
