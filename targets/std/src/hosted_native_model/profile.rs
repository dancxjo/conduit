//! Explicit native profile, exact imported checkpoint, and preparation-owned I/O.
use conduit_ai::{imported_back_admission::*, imported_onnx::*, onnx_metadata::OnnxOperator, *};
use conduit_core::ComputeServiceGuarantee;
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};
pub(super) const ARTIFACT: &str = "adapter/std-ort-1.23.2-cpu-inline-scalar-f32@1";
const DETERMINISM: &str = "deterministic/ieee754-f32-identity@1";
pub(super) fn validate_model(model: &ImportedOnnxResource) -> Result<(), ModelComputeRefusal> {
    let signature = model.resource().signature();
    if model.resource().artifact().precision_profile != "tensor/f32@1"
        || model.sidecar().inputs.len() != 1
        || model.sidecar().outputs.len() != 1
        || model.sidecar().inputs[0].semantic_port != "scalar"
        || model.sidecar().inputs[0].graph_name != "x"
        || model.sidecar().outputs[0].semantic_port != "scalar"
        || model.sidecar().outputs[0].graph_name != "y"
    {
        return Err(ModelComputeRefusal::UnsupportedShape);
    }
    for ports in [
        signature.inputs.get().as_slice(),
        signature.outputs.get().as_slice(),
    ] {
        let [port] = ports else {
            return Err(ModelComputeRefusal::UnsupportedShape);
        };
        let ModelValueConstraint::Tensor(value) = &port.value else {
            return Err(ModelComputeRefusal::UnsupportedShape);
        };
        let tensor = value.constraint();
        if port.identity.get() != "scalar"
            || port.semantic_kind.get() != "data/tensor@1"
            || port.presence != ModelPortPresence::Required
            || tensor.elements.get().as_slice() != [TensorElement::F32]
            || !tensor.axes.get().is_empty()
            || tensor.maximum_bytes != 4
        {
            return Err(ModelComputeRefusal::UnsupportedShape);
        }
    }
    Ok(())
}
pub(super) fn runtime(digest: [u8; 32]) -> ModelComputeRuntimeIdentity {
    ModelComputeRuntimeIdentity {
        provider_name: "CPUExecutionProvider".into(),
        runtime_name: "onnxruntime".into(),
        runtime_version: "1.23.2".into(),
        runtime_build_identity: digest.iter().map(|b| format!("{b:02x}")).collect(),
        adapter_artifact_identity: ARTIFACT.into(),
        device_evidence: "linux-x86_64-glibc/cpu/sequential".into(),
        precision_profile: "tensor/f32@1".into(),
    }
}
pub(super) fn offer(bytes: u64) -> ModelComputeOffer {
    ModelComputeOffer {
        identity: "std/ort-cpu-inline-scalar-f32@1".into(),
        supported_operations: vec![ModelComputeOperation::Inference],
        accepted_formats: vec![ONNX_FORMAT_PROFILE.into()],
        supported_elements: vec![TensorElement::F32],
        solver_profiles: vec![],
        determinism_profiles: vec![DETERMINISM.into()],
        checkpoint_loading: false,
        checkpoint_writing: false,
        limits: ModelComputeLimits {
            maximum_model_bytes: 77,
            maximum_working_memory_bytes: bytes,
            maximum_device_memory_bytes: 0,
            maximum_input_bytes: 4,
            maximum_output_bytes: 4,
            maximum_batch_items: 1,
            maximum_rank: 0,
            maximum_in_flight: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 4,
            cancellation_supported: false,
            compute: ComputeCapacity {
                class: PortableComputeClass::GeneralCpu,
                minimum_lanes: 1,
                preferred_lanes: 1,
                maximum_lanes: 1,
                service: ComputeServiceGuarantee::Shared,
            },
        },
        cache_policy: ModelCachePolicy::NoCache,
    }
}
pub(super) fn requirement(bytes: u64) -> ModelComputeRequirement {
    ModelComputeRequirement {
        operation: ModelComputeOperation::Inference,
        model_format: ONNX_FORMAT_PROFILE.into(),
        elements: vec![TensorElement::F32],
        rank: 0,
        model_bytes: 77,
        working_memory_bytes: bytes,
        device_memory_bytes: 0,
        input_bytes: 4,
        output_bytes: 4,
        batch_items: 1,
        compute_class: PortableComputeClass::GeneralCpu,
        minimum_lanes: 1,
        preferred_lanes: 1,
        maximum_lanes: 1,
        minimum_service: ComputeServiceGuarantee::Shared,
        solver_profile: None,
        determinism_profile: DETERMINISM.into(),
        requires_checkpoint_load: false,
        requires_checkpoint_write: false,
    }
}
pub(super) fn output_template() -> NamedTensorBundle {
    let payload = 0u32.to_le_bytes();
    NamedTensorBundle::from_values(vec![NamedTensor {
        name: "scalar".into(),
        value: TensorValue {
            element: TensorElement::F32,
            dimensions: BoundedSequence::new(),
            axes: BoundedSequence::new(),
            content_digest: tensor_content_digest(&payload),
            backing: TensorBacking::Inline(BoundedBytes::new(&payload).expect("fixed scalar")),
        },
    }])
    .expect("fixed scalar template")
}
pub(super) fn exact(
    model: &ImportedOnnxResource,
    runtime: &ModelComputeRuntimeIdentity,
) -> ExactImportedBackProfile {
    ExactImportedBackProfile {
        profile_identity: "std/ort-cpu-inline-scalar-f32@1".into(),
        implementation_identity: crate::hosted_model_infer::MODEL_INFER_IMPLEMENTATION.into(),
        adapter_artifact_identity: ARTIFACT.into(),
        runtime: runtime.clone(),
        execution_profile: crate::hosted_model_infer::MODEL_INFER_PROFILE.into(),
        checkpoint: ImportedCheckpointIdentity::from_resource(model),
        minimum_ir: 9,
        maximum_ir: 9,
        opsets: vec![ImportedOpsetRange {
            domain: "".into(),
            minimum: 18,
            maximum: 18,
        }],
        operators: vec![OnnxOperator {
            domain: "".into(),
            name: "Identity".into(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_offer_cannot_select_other_tensor_geometry_precision_or_unreserved_memory() {
        let offer = offer(4096);
        let exact = requirement(4096);
        assert_eq!(offer.admits(&exact), Ok(()));
        for case in 0..4 {
            let mut requested = exact.clone();
            match case {
                0 => requested.rank = 1,
                1 => requested.elements = vec![TensorElement::F64],
                2 => requested.working_memory_bytes = 4097,
                _ => requested.requires_checkpoint_write = true,
            }
            assert!(offer.admits(&requested).is_err());
        }
        assert!(!offer.limits.cancellation_supported);
    }
}
