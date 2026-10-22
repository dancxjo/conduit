//! Explicit repository-native proof fixture; not an advertised model Gear.
use super::{NativeBackingGrant, NativeBackingRefusal, NativeCpuProfile, PreparedNativeModel};
use conduit_ai::{imported_onnx::*, *};
use conduit_core::*;
use conduit_data::TensorElement;
use std::sync::Arc;
fn fixture() -> Result<Arc<ImportedOnnxResource>, NativeBackingRefusal> {
    let port = || {
        ModelPortConstraint::from_parts(
            "scalar".into(),
            "data/tensor@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::tensor(
                ModelTensorConstraint::from_parts(vec![TensorElement::F32], vec![], 4).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let signature = ModelSignature::from_parts(
        "proof/native-scalar-identity@1".into(),
        1,
        vec![ModelOperation::Infer],
        vec![port()],
        vec![port()],
    )
    .unwrap();
    let bytes: Arc<[u8]> =
        Arc::from(include_bytes!("../../tests/fixtures/model-scalar-f32.onnx").as_slice());
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(model_content_digest(&bytes)),
        content_profile: ONNX_FORMAT_PROFILE.into(),
        access_class: "proof/native-immutable-model".into(),
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
        handle: "proof/native-model".into(),
        authority_contract: MODEL_READ_AUTHORITY.into(),
        authority_grant: "proof/immutable-model-read".into(),
        maximum_bytes: reference.extent.bytes,
        maximum_items: None,
        availability: ResourceReferenceAvailability::Available,
    };
    let sidecar = OnnxSidecar {
        graph_sha256: super::profile::MODEL_SHA256,
        graph_name: "bounded-scalar-identity".into(),
        signature_identity: signature.semantic_digest().unwrap(),
        inputs: vec![ImportedPortBinding {
            semantic_port: "scalar".into(),
            graph_name: "x".into(),
        }],
        outputs: vec![ImportedPortBinding {
            semantic_port: "scalar".into(),
            graph_name: "y".into(),
        }],
    };
    let artifact = ModelArtifact {
        architecture_profile: "model/scalar-identity@1".into(),
        format_profile: ONNX_FORMAT_PROFILE.into(),
        precision_profile: "tensor/f32@1".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: reference,
    };
    let resource = AdmittedModelResource::adopt(artifact, signature, bytes, &binding)
        .map_err(|_| NativeBackingRefusal::IdentityMismatch)?;
    Ok(Arc::new(
        ImportedOnnxResource::bind(resource, &sidecar)
            .map_err(|_| NativeBackingRefusal::IdentityMismatch)?,
    ))
}
pub fn prove_from_environment() -> Result<(), NativeBackingRefusal> {
    let library = std::env::var("CONDUIT_ORT_LIBRARY")
        .map_err(|_| NativeBackingRefusal::UnsupportedProfile)?;
    let profile = NativeCpuProfile {
        runtime_library: library,
        runtime_sha256: super::profile::RUNTIME_SHA256,
        maximum_calls: 1024,
    };
    let grant = NativeBackingGrant {
        maximum_private_bytes: 64 * 1024 * 1024,
        maximum_fixed_mapping_bytes: 256 * 1024 * 1024,
    };
    let executable = std::env::current_exe().map_err(|_| NativeBackingRefusal::Io)?;
    let resource = fixture()?;
    let mut prepared =
        PreparedNativeModel::prepare(&executable, profile.clone(), grant, resource.clone())?;
    let ledger = prepared.ledger().clone();
    for n in 0..1000 {
        let value = n as f32 / 8.0;
        if prepared.infer_f32(value)?.to_bits() != value.to_bits() {
            return Err(NativeBackingRefusal::InvalidOutput);
        }
    }
    prepared.verify_backing()?;
    let descriptor = prepared.resource().resource().descriptor_identity();
    drop(prepared);
    let insufficient = NativeBackingGrant {
        maximum_private_bytes: 1,
        ..grant
    };
    if !matches!(
        PreparedNativeModel::prepare(&executable, profile.clone(), insufficient, resource.clone()),
        Err(NativeBackingRefusal::InsufficientBacking { .. })
    ) {
        return Err(NativeBackingRefusal::Protocol);
    }
    let insufficient_fixed = NativeBackingGrant {
        maximum_fixed_mapping_bytes: 1,
        ..grant
    };
    if !matches!(
        PreparedNativeModel::prepare(
            &executable,
            profile.clone(),
            insufficient_fixed,
            resource.clone()
        ),
        Err(NativeBackingRefusal::InsufficientBacking { .. })
    ) {
        return Err(NativeBackingRefusal::Protocol);
    }
    let mut pressure = PreparedNativeModel::prepare(&executable, profile.clone(), grant, resource)?;
    for n in 0..101 {
        pressure.infer_f32(n as f32)?;
    }
    if pressure.probe_capacity() != Err(NativeBackingRefusal::ExecutionBoundary) {
        return Err(NativeBackingRefusal::Protocol);
    }
    println!(
        "{}",
        serde_json::json!({"proof":"native-model-backing","profile":super::profile::PROFILE,"exact_cpu_outputs":1000,"insufficient_grant_before_ready":true,"insufficient_fixed_mapping_grant_before_ready":true,"maps_unchanged_after_1000":true,"canonical_model_descriptor":descriptor,"canonical_model_bytes":77,"pressure_without_output":true,"ledger":ledger,"worker_sha256":super::worker::file_digest(&executable)?,"runtime_sha256":profile.runtime_sha256,"model_sha256":super::profile::MODEL_SHA256,"not_established":["ordinary Source/Plan/Play adapter","full session retirement/cancellation","physical residency"]})
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_proof_fixture_retains_checked_scalar_metadata_and_read_authority() {
        let model = fixture().unwrap();
        assert_eq!(model.metadata().inputs[0].element, TensorElement::F32);
        assert!(model.metadata().inputs[0].dimensions.is_empty());
        assert_eq!(model.sidecar().inputs[0].graph_name, "x");
        assert_eq!(model.sidecar().outputs[0].graph_name, "y");
    }
    #[test]
    fn profile_rejects_unreviewed_runtime_and_bound_changes_before_launch() {
        let mut profile = NativeCpuProfile {
            runtime_library: "/not-opened/runtime.so".into(),
            runtime_sha256: super::super::profile::RUNTIME_SHA256,
            maximum_calls: 1,
        };
        assert!(profile.validate().is_ok());
        profile.runtime_sha256[0] ^= 1;
        assert_eq!(
            profile.validate(),
            Err(NativeBackingRefusal::UnsupportedProfile)
        );
    }
}
