use conduit_ai::*;
use conduit_core::ComputeServiceGuarantee;
use conduit_data::TensorElement;

fn limits() -> ModelComputeLimits {
    ModelComputeLimits {
        maximum_model_bytes: 4096,
        maximum_working_memory_bytes: 8192,
        maximum_device_memory_bytes: 0,
        maximum_input_bytes: 256,
        maximum_output_bytes: 256,
        maximum_batch_items: 8,
        maximum_rank: 4,
        maximum_in_flight: 1,
        maximum_queue_items: 2,
        maximum_queue_bytes: 512,
        cancellation_supported: true,
        compute: ComputeCapacity {
            class: PortableComputeClass::GeneralCpu,
            minimum_lanes: 1,
            preferred_lanes: 2,
            maximum_lanes: 4,
            service: ComputeServiceGuarantee::Shared,
        },
    }
}

fn offer() -> ModelComputeOffer {
    ModelComputeOffer {
        identity: "std/reference-model-compute".into(),
        supported_operations: vec![
            ModelComputeOperation::Inference,
            ModelComputeOperation::Encode,
            ModelComputeOperation::Decode,
            ModelComputeOperation::Sample,
            ModelComputeOperation::Score,
            ModelComputeOperation::TrainStep,
            ModelComputeOperation::Evaluate,
            ModelComputeOperation::Checkpoint,
            ModelComputeOperation::IntegrateDynamics,
            ModelComputeOperation::RelationQuery,
        ],
        accepted_formats: vec!["model/reference-linear".into()],
        supported_elements: vec![TensorElement::F32],
        solver_profiles: vec!["fixed-step/reference".into()],
        determinism_profiles: vec!["deterministic/f32".into()],
        checkpoint_loading: true,
        checkpoint_writing: true,
        limits: limits(),
        cache_policy: ModelCachePolicy::bounded(4096, 1).unwrap(),
    }
}

fn requirement(operation: ModelComputeOperation) -> ModelComputeRequirement {
    ModelComputeRequirement {
        operation,
        model_format: "model/reference-linear".into(),
        elements: vec![TensorElement::F32],
        rank: 1,
        model_bytes: 128,
        working_memory_bytes: 512,
        device_memory_bytes: 0,
        input_bytes: 8,
        output_bytes: 8,
        batch_items: 1,
        compute_class: PortableComputeClass::GeneralCpu,
        minimum_lanes: 1,
        preferred_lanes: 2,
        maximum_lanes: 4,
        minimum_service: ComputeServiceGuarantee::Shared,
        solver_profile: None,
        determinism_profile: "deterministic/f32".into(),
        requires_checkpoint_load: false,
        requires_checkpoint_write: false,
    }
}

fn runtime() -> ModelComputeRuntimeIdentity {
    ModelComputeRuntimeIdentity {
        provider_name: "conduit-reference".into(),
        runtime_name: "native-rust".into(),
        runtime_version: "1".into(),
        runtime_build_identity: "build/reference/1".into(),
        adapter_artifact_identity: "adapter/reference/1".into(),
        device_evidence: "host/cpu-observed".into(),
        precision_profile: "f32".into(),
    }
}

#[test]
fn zero_input_is_inference_only_and_offer_capacity_stays_positive() {
    let host = offer();
    let mut request = requirement(ModelComputeOperation::Inference);
    request.input_bytes = 0;
    assert_eq!(host.admits(&request), Ok(()));
    for operation in [
        ModelComputeOperation::Encode,
        ModelComputeOperation::Decode,
        ModelComputeOperation::Sample,
        ModelComputeOperation::Score,
        ModelComputeOperation::TrainStep,
        ModelComputeOperation::Evaluate,
        ModelComputeOperation::Checkpoint,
        ModelComputeOperation::IntegrateDynamics,
        ModelComputeOperation::RelationQuery,
    ] {
        request.operation = operation;
        assert_eq!(
            host.admits(&request),
            Err(ModelComputeRefusal::ResourceBoundExceeded)
        );
    }
    let mut invalid_offer = host.clone();
    invalid_offer.limits.maximum_input_bytes = 0;
    assert_eq!(
        invalid_offer.validate(),
        Err(ModelComputeRefusal::InvalidOffer)
    );
    request.operation = ModelComputeOperation::Inference;
    request.output_bytes = 0;
    assert_eq!(
        host.admits(&request),
        Err(ModelComputeRefusal::ResourceBoundExceeded)
    );
    request.output_bytes = 8;
    request.working_memory_bytes = host.limits.maximum_working_memory_bytes + 1;
    assert_eq!(
        host.admits(&request),
        Err(ModelComputeRefusal::ResourceBoundExceeded)
    );
}

#[test]
fn empty_invocations_consume_queue_items_and_follow_normal_lifecycle() {
    let mut request = requirement(ModelComputeOperation::Inference);
    request.input_bytes = 0;
    let mut session = ModelComputeSession::discovered(offer(), runtime()).unwrap();
    assert_eq!(session.enqueue(0), Err(ModelComputeRefusal::QueueFull));
    assert_eq!(
        session.begin(&request, 0),
        Err(ModelComputeRefusal::InvalidLifecycleTransition)
    );
    session.begin_load([9; 32], 128).unwrap();
    session.begin_warming().unwrap();
    session.ready().unwrap();
    session.enqueue(0).unwrap();
    session.enqueue(0).unwrap();
    assert_eq!(session.enqueue(0), Err(ModelComputeRefusal::QueueFull));
    session.begin(&request, 0).unwrap();
    assert_eq!(
        session.state(),
        ModelComputeLifecycle::Active(ModelComputeOperation::Inference)
    );
    assert_eq!(session.enqueue(0), Err(ModelComputeRefusal::QueueFull));
    session.finish().unwrap();
    session.enqueue(0).unwrap();
    assert_eq!(session.enqueue(0), Err(ModelComputeRefusal::QueueFull));
    session.begin(&request, 0).unwrap();
    session.cancel().unwrap();
    session.begin_unload().unwrap();
    session.shutdown().unwrap();
    assert_eq!(session.state(), ModelComputeLifecycle::Shutdown);
    assert_eq!(
        session.begin(&request, 0),
        Err(ModelComputeRefusal::InvalidLifecycleTransition)
    );
    assert_eq!(session.enqueue(0), Err(ModelComputeRefusal::QueueFull));
}
