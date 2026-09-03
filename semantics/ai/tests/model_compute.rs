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
        cache_policy: ModelCachePolicy::Bounded {
            maximum_loaded_models: 1,
            maximum_loaded_bytes: 4096,
        },
    }
}

fn requirement(operation: ModelComputeOperation) -> ModelComputeRequirement {
    ModelComputeRequirement {
        operation,
        model_format: "model/reference-linear".into(),
        element: TensorElement::F32,
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
fn selection_covers_general_operation_families_and_refuses_known_mismatches() {
    let offer = offer();
    for operation in [
        ModelComputeOperation::Inference,
        ModelComputeOperation::TrainStep,
        ModelComputeOperation::IntegrateDynamics,
        ModelComputeOperation::RelationQuery,
    ] {
        assert_eq!(
            select_model_compute_offer(core::slice::from_ref(&offer), &requirement(operation))
                .unwrap()
                .identity,
            offer.identity
        );
    }
    let mut gpu = requirement(ModelComputeOperation::Inference);
    gpu.compute_class = PortableComputeClass::Accelerator;
    assert_eq!(
        select_model_compute_offer(core::slice::from_ref(&offer), &gpu),
        Err(ModelComputeRefusal::ProviderUnavailable)
    );
    let mut bad_format = requirement(ModelComputeOperation::Inference);
    bad_format.model_format = "onnx".into();
    assert_eq!(
        offer.admits(&bad_format),
        Err(ModelComputeRefusal::UnsupportedFormat)
    );
}

#[test]
fn load_queue_active_cancel_loss_and_unload_are_explicit_and_bounded() {
    let mut session = ModelComputeSession::discovered(offer(), runtime()).unwrap();
    assert_eq!(session.state(), ModelComputeLifecycle::Discovered);
    session.begin_load([9; 32], 128).unwrap();
    session.begin_warming().unwrap();
    session.ready().unwrap();
    session.enqueue(8).unwrap();
    session.enqueue(8).unwrap();
    assert_eq!(session.enqueue(8), Err(ModelComputeRefusal::QueueFull));
    session
        .begin(&requirement(ModelComputeOperation::Inference), 8)
        .unwrap();
    assert_eq!(
        session.state(),
        ModelComputeLifecycle::Active(ModelComputeOperation::Inference)
    );
    session.cancel().unwrap();
    assert_eq!(session.state(), ModelComputeLifecycle::Ready);
    session.provider_lost();
    assert_eq!(session.state(), ModelComputeLifecycle::Lost);
    assert_eq!(
        session.begin(&requirement(ModelComputeOperation::Inference), 0),
        Err(ModelComputeRefusal::InvalidLifecycleTransition)
    );

    let mut unload = ModelComputeSession::discovered(offer(), runtime()).unwrap();
    unload.begin_load([9; 32], 128).unwrap();
    unload.begin_warming().unwrap();
    unload.ready().unwrap();
    unload.begin_unload().unwrap();
    unload.shutdown().unwrap();
    assert_eq!(unload.state(), ModelComputeLifecycle::Shutdown);
}

#[test]
fn every_memory_shape_concurrency_and_checkpoint_bound_fails_closed() {
    let offer = offer();
    let mut request = requirement(ModelComputeOperation::Inference);
    request.working_memory_bytes = offer.limits.maximum_working_memory_bytes + 1;
    assert_eq!(
        offer.admits(&request),
        Err(ModelComputeRefusal::ResourceBoundExceeded)
    );
    request = requirement(ModelComputeOperation::Inference);
    request.rank = offer.limits.maximum_rank + 1;
    assert_eq!(
        offer.admits(&request),
        Err(ModelComputeRefusal::UnsupportedShape)
    );
    request = requirement(ModelComputeOperation::Checkpoint);
    request.requires_checkpoint_write = true;
    let mut no_checkpoint = offer.clone();
    no_checkpoint.checkpoint_writing = false;
    assert_eq!(
        no_checkpoint.admits(&request),
        Err(ModelComputeRefusal::UnsupportedCheckpoint)
    );
    let mut invalid = offer;
    invalid.limits.maximum_in_flight = 2;
    assert_eq!(invalid.validate(), Err(ModelComputeRefusal::InvalidOffer));
}
