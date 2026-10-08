//! Portable, finite commands for one host-owned model session.
//!
//! Canonical semantic bindings and tensor codecs remain the payload authority.
//! Paths, framework/device selection and checkpoint storage belong to host preparation.
use crate::{
    EvaluationReceipt, ModelCheckpoint, TrainStepOutcome, TrainStepRequest, TrainingBatch,
    TrainingCheckpointReceipt, TrainingMetric, TrainingState,
};
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::*;
use conduit_data::TensorValue;
use conduit_plot::rust_binding::NativeRustBinding;
use serde::{Deserialize, Serialize};

pub const MODEL_WORK_KIND: &str = "model/work";
pub const MODEL_WORK_OPERATION: &str = "conduit.host/model-work@1";
pub const MODEL_WORK_IMPLEMENTATION: &str = "std/burn-model-work@1";
pub const MODEL_WORK_PROFILE: &str = "conduit.model/session-work@1";
pub const MODEL_WORK_ARTIFACT: &str = "conduit-std/burn-model-work@1";
pub const MODEL_WORK_RESOURCE_CLASS: &str = "model/session-work";
pub const MODEL_WORK_MAXIMUM_INPUT_BYTES: u32 = 1_048_576;
pub const MODEL_WORK_MAXIMUM_OUTPUT_BYTES: u32 = 1_048_576;
pub const MODEL_WORK_MAXIMUM_TENSORS: usize = 32;
const REVISION: &str = "conduit.ai/model-work@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelWorkRequest {
    pub request_identity: [u8; 32],
    pub session_identity: [u8; 32],
    pub expected_generation: u64,
    pub operation: ModelWorkOperation,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ModelWorkOperation {
    Train {
        #[serde(with = "native")]
        request: TrainStepRequest,
        #[serde(with = "tensors")]
        inputs: Vec<TensorValue>,
        #[serde(with = "tensors")]
        targets: Vec<TensorValue>,
    },
    Evaluate {
        #[serde(with = "native")]
        batch: TrainingBatch,
        #[serde(with = "tensors")]
        inputs: Vec<TensorValue>,
        #[serde(with = "tensors")]
        targets: Vec<TensorValue>,
    },
    Checkpoint {
        #[serde(with = "native_vec")]
        metrics: Vec<TrainingMetric>,
    },
    Export,
    Resume {
        checkpoint_identity: [u8; 32],
    },
    Infer {
        #[serde(with = "tensors")]
        inputs: Vec<TensorValue>,
    },
    ReloadInference {
        checkpoint_identity: [u8; 32],
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelWorkReply {
    pub request_identity: [u8; 32],
    pub session_identity: [u8; 32],
    pub generation: u64,
    pub result: ModelWorkResult,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ModelWorkResult {
    Refused(ModelWorkRefusal),
    Trained(TrainStepOutcome),
    Evaluated(EvaluationReceipt),
    Checkpointed(Box<TrainingCheckpointReceipt>),
    Exported(ModelCheckpoint),
    Resumed(TrainingState),
    Inferred {
        checkpoint_identity: Option<[u8; 32]>,
        #[serde(with = "tensors")]
        outputs: Vec<TensorValue>,
    },
    ReloadedInference {
        checkpoint_identity: [u8; 32],
    },
}
/// Portable refusal data; retain existing semantic refusal identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ModelWorkRefusal {
    Training(#[serde(with = "native")] crate::TrainingRefusal),
    Compute(#[serde(with = "native")] crate::ModelComputeRefusal),
    ResourceBound,
    InvalidDescriptor,
    InvalidTensor,
    IncompatibleCheckpoint,
    CorruptCheckpoint,
    UnsupportedDevice,
    UnsupportedResumeDevice,
    CheckpointIo,
    DurabilityUncertain { checkpoint_identity: [u8; 32] },
    NumericFailure,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelWorkCodecRefusal {
    Bounds,
    Malformed,
    Identity,
}

impl ModelWorkRequest {
    pub fn validate(&self) -> Result<(), ModelWorkCodecRefusal> {
        identities(self.request_identity, self.session_identity)?;
        match &self.operation {
            ModelWorkOperation::Train {
                request,
                inputs,
                targets,
            } => {
                if request.expected_generation != self.expected_generation {
                    return Err(ModelWorkCodecRefusal::Identity);
                }
                tensor_bounds(inputs)?;
                tensor_bounds(targets)?;
            }
            ModelWorkOperation::Evaluate {
                inputs, targets, ..
            } => {
                tensor_bounds(inputs)?;
                tensor_bounds(targets)?;
            }
            ModelWorkOperation::Infer { inputs } => tensor_bounds(inputs)?,
            ModelWorkOperation::Checkpoint { metrics }
                if metrics.len() > crate::MAXIMUM_METRICS =>
            {
                return Err(ModelWorkCodecRefusal::Bounds)
            }
            ModelWorkOperation::Resume {
                checkpoint_identity,
            }
            | ModelWorkOperation::ReloadInference {
                checkpoint_identity,
            } if *checkpoint_identity == [0; 32] => return Err(ModelWorkCodecRefusal::Identity),
            _ => {}
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>, ModelWorkCodecRefusal> {
        self.validate()?;
        encode_bounded(self, MODEL_WORK_MAXIMUM_INPUT_BYTES)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, ModelWorkCodecRefusal> {
        let result: Self = decode_bounded(bytes, MODEL_WORK_MAXIMUM_INPUT_BYTES)?;
        result.validate()?;
        Ok(result)
    }
}
impl ModelWorkReply {
    pub fn validate(&self) -> Result<(), ModelWorkCodecRefusal> {
        identities(self.request_identity, self.session_identity)?;
        match &self.result {
            ModelWorkResult::Refused(ModelWorkRefusal::DurabilityUncertain {
                checkpoint_identity,
            }) if *checkpoint_identity == [0; 32] => return Err(ModelWorkCodecRefusal::Identity),
            ModelWorkResult::Refused(_) => {}
            ModelWorkResult::Trained(TrainStepOutcome::Committed(step)) => {
                state_bounds(&step.state, self.session_identity, self.generation)?;
                if step.receipt.session_identity != self.session_identity
                    || step.receipt.generation != self.generation
                {
                    return Err(ModelWorkCodecRefusal::Identity);
                }
                metrics_bounds(&step.receipt.metrics)?;
                text_bound(&step.receipt.realization_identity)?;
            }
            ModelWorkResult::Trained(TrainStepOutcome::NotCommitted {
                retained_generation,
                ..
            }) => {
                if *retained_generation != self.generation {
                    return Err(ModelWorkCodecRefusal::Identity);
                }
            }
            ModelWorkResult::Evaluated(receipt) => {
                if receipt.session_identity != self.session_identity
                    || receipt.state_generation != self.generation
                {
                    return Err(ModelWorkCodecRefusal::Identity);
                }
                metrics_bounds(&receipt.metrics)?;
                text_bound(&receipt.state_identity)?;
                text_bound(&receipt.realization_identity)?;
            }
            ModelWorkResult::Checkpointed(receipt) => {
                if receipt.session_identity != self.session_identity {
                    return Err(ModelWorkCodecRefusal::Identity);
                }
                checkpoint_bounds(&receipt.checkpoint, self.generation)?;
                metrics_bounds(&receipt.metric_summaries)?;
                text_bound(&receipt.objective_profile)?;
                let realization = &receipt.realization;
                for text in [
                    &realization.implementation_identity,
                    &realization.runtime_name,
                    &realization.runtime_version,
                    &realization.runtime_build_identity,
                    &realization.device_profile,
                    &realization.format_profile,
                    &realization.precision_profile,
                    &realization.deterministic_profile,
                ] {
                    text_bound(text)?;
                }
            }
            ModelWorkResult::Exported(checkpoint) => {
                checkpoint_bounds(checkpoint, self.generation)?
            }
            ModelWorkResult::Resumed(state) => {
                state_bounds(state, self.session_identity, self.generation)?
            }
            ModelWorkResult::Inferred {
                outputs,
                checkpoint_identity,
            } => {
                tensor_bounds(outputs)?;
                if *checkpoint_identity == Some([0; 32]) {
                    return Err(ModelWorkCodecRefusal::Identity);
                }
            }
            ModelWorkResult::ReloadedInference {
                checkpoint_identity,
            } => {
                if *checkpoint_identity == [0; 32] {
                    return Err(ModelWorkCodecRefusal::Identity);
                }
            }
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>, ModelWorkCodecRefusal> {
        self.validate()?;
        encode_bounded(self, MODEL_WORK_MAXIMUM_OUTPUT_BYTES)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, ModelWorkCodecRefusal> {
        let result: Self = decode_bounded(bytes, MODEL_WORK_MAXIMUM_OUTPUT_BYTES)?;
        result.validate()?;
        Ok(result)
    }
}

/// Conservative wire bound for every mutating operation's receipt.
///
/// Native metric and randomness layouts have fixed canonical headers and bounded
/// text payloads. A JSON byte-array costs at most four bytes per canonical byte.
/// The remaining receipt variants have fewer than 64 named scalar fields/digests,
/// at most 16 text fields of 128 bytes (six-byte JSON escapes), and one resource
/// reference. 64KiB covers their punctuation, field names, scalars and text with
/// substantial headroom. Tensor outputs are excluded: inference does not mutate
/// the training generation and may safely refuse an oversized output.
pub fn model_work_mutating_reply_bytes_bound() -> Result<usize, ModelWorkCodecRefusal> {
    let metric = TrainingMetric::new(
        crate::TrainingObjectiveIdentity::new("x".repeat(128))
            .map_err(|_| ModelWorkCodecRefusal::Malformed)?,
        i64::MIN,
    )
    .map_err(|_| ModelWorkCodecRefusal::Malformed)?;
    let metric_bytes = metric
        .encode()
        .map_err(|_| ModelWorkCodecRefusal::Malformed)?
        .len();
    let randomness = crate::RandomnessProfile::provider_chosen("x".repeat(128), u64::MAX)
        .map_err(|_| ModelWorkCodecRefusal::Malformed)?;
    let randomness_bytes = randomness
        .encode()
        .map_err(|_| ModelWorkCodecRefusal::Malformed)?
        .len();
    Ok(crate::MAXIMUM_METRICS * (4 * metric_bytes + 1)
        + 4 * randomness_bytes
        + 4 * conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES
        + 65_536)
}

fn text_bound(text: &str) -> Result<(), ModelWorkCodecRefusal> {
    if text.is_empty() || text.len() > crate::MAXIMUM_MODEL_IDENTITY_BYTES {
        Err(ModelWorkCodecRefusal::Bounds)
    } else {
        Ok(())
    }
}
fn metrics_bounds(values: &[TrainingMetric]) -> Result<(), ModelWorkCodecRefusal> {
    if values.len() > crate::MAXIMUM_METRICS {
        Err(ModelWorkCodecRefusal::Bounds)
    } else {
        Ok(())
    }
}
fn state_bounds(
    state: &TrainingState,
    session: [u8; 32],
    generation: u64,
) -> Result<(), ModelWorkCodecRefusal> {
    if state.session_identity != session
        || state.model.generation != generation
        || state.model.base_artifact_identity == [0; 32]
        || state.model.state_schema_version == 0
    {
        return Err(ModelWorkCodecRefusal::Identity);
    }
    text_bound(&state.model.state_identity)
}
fn checkpoint_bounds(
    checkpoint: &ModelCheckpoint,
    generation: u64,
) -> Result<(), ModelWorkCodecRefusal> {
    if checkpoint.generation != generation
        || checkpoint.base_artifact_identity == [0; 32]
        || checkpoint.state_schema_version == 0
    {
        return Err(ModelWorkCodecRefusal::Identity);
    }
    checkpoint
        .content
        .validate()
        .map_err(|_| ModelWorkCodecRefusal::Malformed)?;
    text_bound(&checkpoint.architecture_profile)
}
fn identities(request: [u8; 32], session: [u8; 32]) -> Result<(), ModelWorkCodecRefusal> {
    if request == [0; 32] || session == [0; 32] {
        Err(ModelWorkCodecRefusal::Identity)
    } else {
        Ok(())
    }
}
fn tensor_bounds(values: &[TensorValue]) -> Result<(), ModelWorkCodecRefusal> {
    if values.is_empty() || values.len() > MODEL_WORK_MAXIMUM_TENSORS {
        return Err(ModelWorkCodecRefusal::Bounds);
    }
    for value in values {
        value
            .validate()
            .map_err(|_| ModelWorkCodecRefusal::Malformed)?;
    }
    Ok(())
}
fn encode_bounded<T: Serialize>(value: &T, maximum: u32) -> Result<Vec<u8>, ModelWorkCodecRefusal> {
    let bytes = serde_json::to_vec(value).map_err(|_| ModelWorkCodecRefusal::Malformed)?;
    if bytes.len() > maximum as usize {
        return Err(ModelWorkCodecRefusal::Bounds);
    }
    Ok(bytes)
}
fn decode_bounded<T: for<'de> Deserialize<'de> + Serialize>(
    bytes: &[u8],
    maximum: u32,
) -> Result<T, ModelWorkCodecRefusal> {
    // Refuse before deserialization, including before nested payload allocations.
    if bytes.len() > maximum as usize {
        return Err(ModelWorkCodecRefusal::Bounds);
    }
    let value: T = serde_json::from_slice(bytes).map_err(|_| ModelWorkCodecRefusal::Malformed)?;
    // One canonical spelling; reject unknown fields, duplicate fields and trailing material.
    if encode_bounded(&value, maximum)?.as_slice() != bytes {
        return Err(ModelWorkCodecRefusal::Malformed);
    }
    Ok(value)
}

#[path = "model_work_codec.rs"]
mod codec;
use codec::tensors;
pub(crate) use codec::{native, native_vec, resource};

pub fn model_work_kind() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(MODEL_WORK_KIND),
        kind_contract_revision: KindIdentity::from(REVISION),
        inputs: vec![work_port(
            "request",
            "model/work-request@1",
            PortDirection::Input,
        )],
        outputs: vec![work_port(
            "reply",
            "model/work-reply@1",
            PortDirection::Output,
        )],
        configuration: Vec::new(),
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![
            FrontValueContract {
                location: FrontValueLocation::Input(port_id("request")),
                contract: CheckedValueContract::new(
                    kind_id("model/work-request@1"),
                    MODEL_WORK_MAXIMUM_INPUT_BYTES,
                    vec![],
                )
                .expect("finite request envelope"),
            },
            FrontValueContract {
                location: FrontValueLocation::Output(port_id("reply")),
                contract: CheckedValueContract::new(
                    kind_id("model/work-reply@1"),
                    MODEL_WORK_MAXIMUM_OUTPUT_BYTES,
                    vec![],
                )
                .expect("finite reply envelope"),
            },
        ])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MODEL_WORK_MAXIMUM_INPUT_BYTES,
        },
    }
}
fn work_port(name: &str, value: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    }
}
pub fn model_work_offer(process_identity: &str) -> Result<CapabilityOffer, String> {
    if process_identity.is_empty() || process_identity.len() > 128 {
        return Err("invalid model process identity".into());
    }
    let kind = model_work_kind();
    Ok(conduit_core::capability_offer_from_parts! {
        startup_parameters: kind.startup_parameters.clone(), shorthand: None,
        capability_id: CapabilityId::from(format!("model/work/process/{process_identity}")),
        kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(),
        inputs: kind.inputs.clone(), outputs: kind.outputs.clone(), semantic_contract: kind.semantic_contract(),
        implementation: ImplementationOffer { execution_profile_id: ExecutionProfileId::from(MODEL_WORK_PROFILE), implementation_id: ImplementationId::from(MODEL_WORK_IMPLEMENTATION), artifact_id: ArtifactId::from(MODEL_WORK_ARTIFACT) },
        host_calls: vec![HostCallRequirement { contract_id: HostCallContractId::from(MODEL_WORK_OPERATION), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: MODEL_WORK_MAXIMUM_INPUT_BYTES, maximum_output_bytes: MODEL_WORK_MAXIMUM_OUTPUT_BYTES }],
        resource_requirements: vec![resource_requirement(MODEL_WORK_RESOURCE_CLASS, 1)], authority_requirements: Vec::new(), limits: kind.limits,
    })
}
#[cfg(feature = "plot-catalog")]
pub fn install_model_work_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    // Opaque wire-envelope aliases, not structural JSON/native Type declarations.
    // The Host decoder checks canonical spelling and authoritative typed payloads.
    startup.insert_value_kind_alias("ModelWorkRequestBytes", kind_id("model/work-request@1"))?;
    startup.insert_value_kind_alias("ModelWorkReplyBytes", kind_id("model/work-reply@1"))?;
    startup.insert(conduit_plot::KindSignature {
        kind: MODEL_WORK_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(model_work_kind())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "model_work_contract_tests.rs"]
mod tests;
