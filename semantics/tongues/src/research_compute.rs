//! Exact std Host compute offer used by the reference research experiment realization.

use crate::{ResearchError, TRAINING_WORK_BOUND};
use conduit_ai::{
    ComputeCapacity, ModelCachePolicy, ModelComputeLimits, ModelComputeOffer,
    ModelComputeOperation, ModelComputeRequirement, PortableComputeClass,
};
use conduit_core::ComputeServiceGuarantee;
use conduit_data::TensorElement;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ComputeEvidence {
    pub offer_identity: String,
    pub runtime: String,
    pub precision: String,
    pub admitted_operations: Vec<String>,
    pub maximum_work_units: u64,
    pub maximum_working_memory_bytes: u64,
}

pub(crate) fn std_compute_evidence() -> Result<ComputeEvidence, ResearchError> {
    let offer = std_compute_offer();
    offer
        .validate()
        .map_err(|_| ResearchError::InvalidSignature)?;
    let operations = [
        ModelComputeOperation::TrainStep,
        ModelComputeOperation::Evaluate,
        ModelComputeOperation::Checkpoint,
        ModelComputeOperation::IntegrateDynamics,
        ModelComputeOperation::RelationQuery,
    ];
    for operation in operations {
        offer
            .admits(&compute_requirement(operation))
            .map_err(|_| ResearchError::InvalidSignature)?;
    }
    Ok(ComputeEvidence {
        offer_identity: offer.identity,
        runtime: format!("native-rust/{}", env!("CARGO_PKG_VERSION")),
        precision: "f64".into(),
        admitted_operations: operations
            .iter()
            .map(|value| format!("{value:?}"))
            .collect(),
        maximum_work_units: TRAINING_WORK_BOUND,
        maximum_working_memory_bytes: offer.limits.maximum_working_memory_bytes,
    })
}

fn std_compute_offer() -> ModelComputeOffer {
    ModelComputeOffer {
        identity: "conduit.std/tongues-reference-compute@1".into(),
        supported_operations: vec![
            ModelComputeOperation::TrainStep,
            ModelComputeOperation::Evaluate,
            ModelComputeOperation::Checkpoint,
            ModelComputeOperation::IntegrateDynamics,
            ModelComputeOperation::RelationQuery,
        ],
        accepted_formats: vec!["model/tongues-paired-linear-json@1".into()],
        supported_elements: vec![TensorElement::F64],
        solver_profiles: vec!["recurrent/discrete-linear@1".into()],
        determinism_profiles: vec!["deterministic/seeded-f64@1".into()],
        checkpoint_loading: true,
        checkpoint_writing: true,
        limits: ModelComputeLimits {
            maximum_model_bytes: 65_536,
            maximum_working_memory_bytes: 1_048_576,
            maximum_device_memory_bytes: 0,
            maximum_input_bytes: 65_536,
            maximum_output_bytes: 65_536,
            maximum_batch_items: 128,
            maximum_rank: 2,
            maximum_in_flight: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 65_536,
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
            maximum_loaded_bytes: 65_536,
        },
    }
}

fn compute_requirement(operation: ModelComputeOperation) -> ModelComputeRequirement {
    ModelComputeRequirement {
        operation,
        model_format: "model/tongues-paired-linear-json@1".into(),
        element: TensorElement::F64,
        rank: 2,
        model_bytes: 65_536,
        working_memory_bytes: 1_048_576,
        device_memory_bytes: 0,
        input_bytes: 65_536,
        output_bytes: 65_536,
        batch_items: 128,
        compute_class: PortableComputeClass::GeneralCpu,
        minimum_lanes: 1,
        preferred_lanes: 1,
        maximum_lanes: 1,
        minimum_service: ComputeServiceGuarantee::Shared,
        solver_profile: Some("recurrent/discrete-linear@1".into()),
        determinism_profile: "deterministic/seeded-f64@1".into(),
        requires_checkpoint_load: true,
        requires_checkpoint_write: true,
    }
}
