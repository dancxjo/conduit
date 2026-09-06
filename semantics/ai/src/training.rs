//! Provider-neutral, bounded model training and evaluation semantics.

use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{PlannedStateBoundary, StateContinuation};
use conduit_data::{DatasetDescriptor, DatasetSplitMembership};

use crate::{ModelArtifact, MutableModelState, RandomnessProfile};

#[path = "training_request.rs"]
mod request;
#[path = "training_validation.rs"]
mod validation;
pub use request::*;
#[path = "training_lifecycle.rs"]
mod lifecycle;
pub use lifecycle::*;
#[path = "training_receipt.rs"]
mod receipt;
pub use receipt::*;
use validation::{
    has_duplicate_text, nonzero, text, validate_evaluation_policy, validate_interval_policy,
    validate_metrics, validate_objectives,
};

pub const MAXIMUM_TRAINING_OBJECTIVES: usize = 32;
pub const MAXIMUM_BATCH_EXAMPLES: usize = 4096;
pub const MAXIMUM_BATCH_MODALITIES: usize = 32;
pub const MAXIMUM_METRICS: usize = 64;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ObjectiveParticipation {
    Optimize,
    ObserveOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingObjective {
    pub role: String,
    /// Fixed-point weight in millionths. Observe-only metrics may use zero.
    pub weight_millionths: u64,
    pub configuration_identity: String,
    pub output_identity: String,
    pub participation: ObjectiveParticipation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingModalityPolicy {
    Reject,
    PermitDeclared { optional_modalities: Vec<String> },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BatchOrder {
    Stable,
    Shuffled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingBatch {
    pub identity: [u8; 32],
    pub dataset_identity: [u8; 32],
    pub split_identity: String,
    pub example_identities: Vec<[u8; 32]>,
    pub present_modalities: Vec<String>,
    pub encoded_bytes: u64,
    pub order: BatchOrder,
    pub stochastic_seed: Option<u64>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TrainingResourceEnvelope {
    pub model_bytes: u64,
    pub working_memory_bytes: u64,
    pub compute_lanes: u32,
    pub maximum_batch_items: u32,
    pub maximum_batch_bytes: u64,
    pub maximum_steps: u64,
    pub maximum_work_units: u64,
    pub maximum_checkpoint_bytes: u64,
    pub maximum_in_flight_steps: u16,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CheckpointPolicy {
    None,
    EverySteps(u64),
    AtCompletion,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EvaluationPolicy {
    None,
    EverySteps(u64),
    AtCompletion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingSession {
    pub identity: [u8; 32],
    pub base_artifact_identity: [u8; 32],
    pub base_checkpoint_identity: Option<[u8; 32]>,
    pub dataset_manifest_identity: [u8; 32],
    pub split_membership_identity: [u8; 32],
    pub objective_profile: String,
    pub objectives: Vec<TrainingObjective>,
    pub randomness: RandomnessProfile,
    pub precision_profile: String,
    pub model_modalities: Vec<String>,
    pub missing_modality_policy: MissingModalityPolicy,
    pub resources: TrainingResourceEnvelope,
    pub checkpoint_policy: CheckpointPolicy,
    pub evaluation_policy: EvaluationPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingState {
    pub session_identity: [u8; 32],
    pub model: MutableModelState,
    pub initial_generation: u64,
    pub completed_steps: u64,
    pub consumed_work_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingMetric {
    pub output_identity: String,
    pub value_millionths: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTrainingRealization {
    pub implementation_identity: String,
    pub runtime_name: String,
    pub runtime_version: String,
    pub runtime_build_identity: String,
    pub device_profile: String,
    pub format_profile: String,
    pub precision_profile: String,
    pub deterministic_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainStepRequest {
    pub step: u64,
    pub expected_generation: u64,
    pub batch: TrainingBatch,
    pub admitted_work_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostStepCandidate {
    pub state_identity: String,
    pub state_schema_version: u32,
    pub generation: u64,
    pub metrics: Vec<TrainingMetric>,
    pub consumed_work_units: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TrainStepFailure {
    Cancelled,
    ResourceExhausted,
    ProviderLost,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostStepTerminal {
    Candidate(HostStepCandidate),
    NoCommit(TrainStepFailure),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TrainingRefusal {
    InvalidIdentity,
    InvalidArtifact,
    InvalidSession,
    InvalidObjective,
    TooManyObjectives,
    NoOptimizationObjective,
    InvalidResourceEnvelope,
    InvalidPolicy,
    InvalidSplit,
    InvalidBatch,
    BatchBoundExceeded,
    MissingRequiredModality,
    UnexpectedSeed,
    StepBoundExceeded,
    StaleState,
    InvalidCandidate,
    WorkBoundExceeded,
    InvalidMetric,
    DuplicateMetric,
    InvalidRealization,
    CheckpointBoundExceeded,
    CheckpointNotScheduled,
    EvaluationNotScheduled,
    InvalidLifecycleTransition,
}

impl TrainingSession {
    pub fn validate(
        &self,
        artifact: &ModelArtifact,
        dataset: &DatasetDescriptor,
        split: &DatasetSplitMembership,
    ) -> Result<(), TrainingRefusal> {
        nonzero(self.identity)?;
        nonzero(self.dataset_manifest_identity)?;
        nonzero(self.split_membership_identity)?;
        text(&self.objective_profile)?;
        text(&self.precision_profile)?;
        if self.base_artifact_identity != artifact.content_identity()
            || self.precision_profile != artifact.precision_profile
            || self.base_checkpoint_identity == Some([0; 32])
            || self.base_checkpoint_identity == Some(artifact.content_identity())
        {
            return Err(TrainingRefusal::InvalidArtifact);
        }
        artifact
            .content
            .validate()
            .map_err(|_| TrainingRefusal::InvalidArtifact)?;
        if artifact.state_schema_version == 0
            || artifact.content.extent.bytes == 0
            || artifact.content.content_profile.as_str() != artifact.format_profile
        {
            return Err(TrainingRefusal::InvalidArtifact);
        }
        dataset
            .validate_membership(split)
            .map_err(|_| TrainingRefusal::InvalidSplit)?;
        if self.dataset_manifest_identity != dataset.manifest.identity.digest()
            || self.split_membership_identity
                != split
                    .semantic_digest()
                    .map_err(|_| TrainingRefusal::InvalidSplit)?
        {
            return Err(TrainingRefusal::InvalidSplit);
        }
        validate_objectives(&self.objectives)?;
        self.resources.validate()?;
        if self.resources.model_bytes < artifact.content.extent.bytes
            || self.model_modalities.is_empty()
            || self.model_modalities.len() > MAXIMUM_BATCH_MODALITIES
        {
            return Err(TrainingRefusal::InvalidResourceEnvelope);
        }
        for modality in &self.model_modalities {
            text(modality)?;
        }
        if self
            .model_modalities
            .iter()
            .enumerate()
            .any(|(index, value)| self.model_modalities[index + 1..].contains(value))
        {
            return Err(TrainingRefusal::InvalidSession);
        }
        if let RandomnessProfile::ProviderChosen { nonce, .. } = &self.randomness {
            text(nonce)?;
        }
        validate_interval_policy(self.checkpoint_policy)?;
        validate_evaluation_policy(self.evaluation_policy)?;
        match &self.missing_modality_policy {
            MissingModalityPolicy::Reject => {}
            MissingModalityPolicy::PermitDeclared {
                optional_modalities,
            } => {
                if optional_modalities.is_empty()
                    || optional_modalities.len() > MAXIMUM_BATCH_MODALITIES
                    || has_duplicate_text(optional_modalities)
                {
                    return Err(TrainingRefusal::InvalidSession);
                }
                for modality in optional_modalities {
                    text(modality)?;
                    if !self.model_modalities.contains(modality) {
                        return Err(TrainingRefusal::InvalidSession);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn planned_state_boundary(
        &self,
        artifact: &ModelArtifact,
        state: &TrainingState,
        maximum_state_bytes: u32,
    ) -> Result<PlannedStateBoundary, TrainingRefusal> {
        state.validate_for(self, artifact)?;
        if maximum_state_bytes < 24 {
            return Err(TrainingRefusal::InvalidSession);
        }
        let mut initial_value = Vec::with_capacity(24);
        initial_value.extend_from_slice(&state.model.generation.to_le_bytes());
        initial_value.extend_from_slice(&state.completed_steps.to_le_bytes());
        initial_value.extend_from_slice(&state.consumed_work_units.to_le_bytes());
        Ok(PlannedStateBoundary {
            state_id: state.model.state_identity.clone().into(),
            gear_id: "ai/train-step".into(),
            value_kind: "ai/training-state@1".into(),
            initial_value,
            retained: None,
            maximum_value_bytes: maximum_state_bytes,
            continuation: StateContinuation::MaximumTransitions(self.resources.maximum_steps),
        })
    }

    pub fn commit_step(
        &self,
        commit: TrainStepCommit<'_>,
    ) -> Result<TrainStepOutcome, TrainingRefusal> {
        let TrainStepCommit {
            artifact,
            dataset,
            split,
            state,
            request,
            terminal,
            realization,
        } = commit;
        self.validate(artifact, dataset, split)?;
        state.validate_for(self, artifact)?;
        realization.validate_for(self, artifact)?;
        request.validate_for(self, split)?;
        if request.expected_generation != state.model.generation
            || request.step != state.completed_steps.saturating_add(1)
        {
            return Err(TrainingRefusal::StaleState);
        }
        let candidate = match terminal {
            HostStepTerminal::Candidate(candidate) => candidate,
            HostStepTerminal::NoCommit(failure) => {
                return Ok(TrainStepOutcome::NotCommitted {
                    failure,
                    retained_generation: state.model.generation,
                })
            }
        };
        candidate.validate_for(self, &state.model, request)?;
        let next_model = MutableModelState {
            base_artifact_identity: state.model.base_artifact_identity,
            state_identity: candidate.state_identity.clone(),
            state_schema_version: candidate.state_schema_version,
            generation: candidate.generation,
        };
        next_model
            .validate(artifact)
            .map_err(|_| TrainingRefusal::InvalidCandidate)?;
        let consumed_work_units = state
            .consumed_work_units
            .checked_add(candidate.consumed_work_units)
            .ok_or(TrainingRefusal::WorkBoundExceeded)?;
        if consumed_work_units > self.resources.maximum_work_units {
            return Err(TrainingRefusal::WorkBoundExceeded);
        }
        let next = TrainingState {
            session_identity: state.session_identity,
            model: next_model,
            initial_generation: state.initial_generation,
            completed_steps: request.step,
            consumed_work_units,
        };
        let receipt = TrainStepReceipt {
            session_identity: self.identity,
            step: request.step,
            prior_generation: state.model.generation,
            generation: next.model.generation,
            batch_identity: request.batch.identity,
            metrics: candidate.metrics,
            consumed_work_units: candidate.consumed_work_units,
            realization_identity: realization.implementation_identity.clone(),
        };
        Ok(TrainStepOutcome::Committed(Box::new(
            CommittedTrainingStep {
                state: next,
                receipt,
            },
        )))
    }

    pub fn evaluate(
        &self,
        request: EvaluationRequest<'_>,
    ) -> Result<EvaluationReceipt, TrainingRefusal> {
        let EvaluationRequest {
            artifact,
            dataset,
            split,
            state,
            batch,
            metrics,
            consumed_work_units,
            realization,
        } = request;
        self.validate(artifact, dataset, split)?;
        state.validate_for(self, artifact)?;
        realization.validate_for(self, artifact)?;
        batch.validate_for(self, split)?;
        validate_metrics(&metrics, &self.objectives)?;
        if consumed_work_units == 0 || consumed_work_units > self.resources.maximum_work_units {
            return Err(TrainingRefusal::WorkBoundExceeded);
        }
        let scheduled = match self.evaluation_policy {
            EvaluationPolicy::None => false,
            EvaluationPolicy::EverySteps(interval) => {
                state.completed_steps.is_multiple_of(interval)
            }
            EvaluationPolicy::AtCompletion => state.completed_steps == self.resources.maximum_steps,
        };
        if !scheduled {
            return Err(TrainingRefusal::EvaluationNotScheduled);
        }
        Ok(EvaluationReceipt {
            session_identity: self.identity,
            batch_identity: batch.identity,
            state_identity: state.model.state_identity.clone(),
            state_generation: state.model.generation,
            metrics,
            consumed_work_units,
            realization_identity: realization.implementation_identity.clone(),
        })
    }

    pub fn checkpoint(
        &self,
        request: CheckpointRequest<'_>,
    ) -> Result<TrainingCheckpointReceipt, TrainingRefusal> {
        let CheckpointRequest {
            artifact,
            dataset,
            split,
            state,
            checkpoint,
            metric_summaries,
            realization,
        } = request;
        self.validate(artifact, dataset, split)?;
        state.validate_for(self, artifact)?;
        realization.validate_for(self, artifact)?;
        checkpoint
            .validate(artifact)
            .map_err(|_| TrainingRefusal::InvalidCandidate)?;
        if checkpoint.generation != state.model.generation {
            return Err(TrainingRefusal::StaleState);
        }
        if checkpoint.content.extent.bytes > self.resources.maximum_checkpoint_bytes {
            return Err(TrainingRefusal::CheckpointBoundExceeded);
        }
        validate_metrics(&metric_summaries, &self.objectives)?;
        let scheduled = match self.checkpoint_policy {
            CheckpointPolicy::None => false,
            CheckpointPolicy::EverySteps(interval) => {
                state.completed_steps.is_multiple_of(interval)
            }
            CheckpointPolicy::AtCompletion => state.completed_steps == self.resources.maximum_steps,
        };
        if !scheduled {
            return Err(TrainingRefusal::CheckpointNotScheduled);
        }
        Ok(TrainingCheckpointReceipt {
            session_identity: self.identity,
            session_descriptor_identity: self.semantic_digest(artifact, dataset, split)?,
            base_artifact_identity: self.base_artifact_identity,
            dataset_manifest_identity: self.dataset_manifest_identity,
            split_membership_identity: self.split_membership_identity,
            objective_profile: self.objective_profile.clone(),
            randomness: self.randomness.clone(),
            realization: realization.clone(),
            completed_steps: state.completed_steps,
            consumed_work_units: state.consumed_work_units,
            metric_summaries,
            checkpoint,
        })
    }
}
