use conduit_ai::*;
use conduit_core::ComputeServiceGuarantee;
use conduit_data::{
    tensor_content_digest, TensorAxis, TensorAxisRole, TensorBacking, TensorElement, TensorValue,
};
use conduit_std_host::hosted_model_compute::{
    LinearF32ModelAdapter, ModelComputeAdapter, ModelComputeAdapterTerminal,
    ModelComputeInvocation, ReferenceModelComputeAdapter,
};

fn offer() -> ModelComputeOffer {
    ModelComputeOffer {
        identity: "std/linear-f32".into(),
        supported_operations: vec![ModelComputeOperation::Inference],
        accepted_formats: vec!["model/reference-linear".into()],
        supported_elements: vec![TensorElement::F32],
        solver_profiles: vec![],
        determinism_profiles: vec!["deterministic/f32".into()],
        checkpoint_loading: false,
        checkpoint_writing: false,
        limits: ModelComputeLimits {
            maximum_model_bytes: 1024,
            maximum_working_memory_bytes: 1024,
            maximum_device_memory_bytes: 0,
            maximum_input_bytes: 8,
            maximum_output_bytes: 8,
            maximum_batch_items: 1,
            maximum_rank: 1,
            maximum_in_flight: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 8,
            cancellation_supported: true,
            compute: ComputeCapacity {
                class: PortableComputeClass::GeneralCpu,
                minimum_lanes: 1,
                preferred_lanes: 1,
                maximum_lanes: 1,
                service: ComputeServiceGuarantee::Shared,
            },
        },
        cache_policy: ModelCachePolicy::Bounded {
            maximum_loaded_models: 1,
            maximum_loaded_bytes: 1024,
        },
    }
}

fn runtime(adapter: &str) -> ModelComputeRuntimeIdentity {
    ModelComputeRuntimeIdentity {
        provider_name: "conduit-std".into(),
        runtime_name: "native-rust".into(),
        runtime_version: "1".into(),
        runtime_build_identity: format!("build/{adapter}/1"),
        adapter_artifact_identity: format!("adapter/{adapter}/1"),
        device_evidence: "observed/cpu".into(),
        precision_profile: "f32".into(),
    }
}

fn input(values: [f32; 2]) -> TensorValue {
    let bytes = values
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    TensorValue {
        element: TensorElement::F32,
        dimensions: vec![2],
        axes: vec![TensorAxis {
            role: TensorAxisRole::Feature,
            identity: Some("linear-feature".into()),
            unit: None,
        }],
        content_digest: tensor_content_digest(&bytes),
        backing: TensorBacking::Inline(bytes),
    }
}

fn invocation(artifact: [u8; 32], values: [f32; 2]) -> ModelComputeInvocation {
    ModelComputeInvocation {
        request_identity: [31; 32],
        artifact_identity: artifact,
        requirement: ModelComputeRequirement {
            operation: ModelComputeOperation::Inference,
            model_format: "model/reference-linear".into(),
            element: TensorElement::F32,
            rank: 1,
            model_bytes: 64,
            working_memory_bytes: 64,
            device_memory_bytes: 0,
            input_bytes: 8,
            output_bytes: 8,
            batch_items: 1,
            compute_class: PortableComputeClass::GeneralCpu,
            minimum_lanes: 1,
            preferred_lanes: 1,
            maximum_lanes: 1,
            minimum_service: ComputeServiceGuarantee::Shared,
            solver_profile: None,
            determinism_profile: "deterministic/f32".into(),
            requires_checkpoint_load: false,
            requires_checkpoint_write: false,
        },
        input: input(values),
    }
}

fn values(tensor: &TensorValue) -> [f32; 2] {
    let TensorBacking::Inline(bytes) = &tensor.backing else {
        panic!()
    };
    [
        f32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
    ]
}

#[test]
fn reference_adapter_proves_exact_boundary_and_real_linear_model_computes() {
    let artifact = [44; 32];
    let mut reference = ReferenceModelComputeAdapter::new(offer(), runtime("reference")).unwrap();
    reference.load(artifact, 64).unwrap();
    let ModelComputeAdapterTerminal::Produced(reference_result) =
        reference.execute(invocation(artifact, [2.0, 3.0]))
    else {
        panic!("reference adapter must produce")
    };
    assert_eq!(values(&reference_result.output), [2.0, 3.0]);
    assert_eq!(reference_result.artifact_identity, artifact);
    assert_ne!(
        reference_result.runtime.adapter_artifact_identity,
        reference_result
            .artifact_identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    reference.unload().unwrap();

    let mut linear = LinearF32ModelAdapter::new(
        offer(),
        runtime("linear"),
        [[2.0, 0.0], [0.0, -1.0]],
        [1.0, 4.0],
    )
    .unwrap();
    linear.load(artifact, 64).unwrap();
    let ModelComputeAdapterTerminal::Produced(result) =
        linear.execute(invocation(artifact, [2.0, 3.0]))
    else {
        panic!("real linear model must produce")
    };
    assert_eq!(values(&result.output), [5.0, 1.0]);
    assert_eq!(result.consumed_work_units, 8);
    assert_eq!(result.input_identity, input([2.0, 3.0]).content_digest);
    assert_eq!(result.runtime.adapter_artifact_identity, "adapter/linear/1");
}

#[test]
fn wrong_loaded_model_and_known_resource_mismatch_refuse_before_compute() {
    let mut linear = LinearF32ModelAdapter::new(
        offer(),
        runtime("linear"),
        [[1.0, 0.0], [0.0, 1.0]],
        [0.0, 0.0],
    )
    .unwrap();
    linear.load([44; 32], 64).unwrap();
    assert_eq!(
        linear.execute(invocation([45; 32], [1.0, 2.0])),
        ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::ProviderUnavailable)
    );
    let mut oversized = invocation([44; 32], [1.0, 2.0]);
    oversized.requirement.working_memory_bytes = 2048;
    assert_eq!(
        linear.execute(oversized),
        ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::ResourceBoundExceeded)
    );
}
