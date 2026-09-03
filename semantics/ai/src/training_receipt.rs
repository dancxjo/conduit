//! Durable outcomes from bounded training and evaluation work.

use alloc::{boxed::Box, string::String, vec::Vec};

use super::{HostTrainingRealization, TrainStepFailure, TrainingMetric, TrainingState};
use crate::{ModelCheckpoint, RandomnessProfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainStepReceipt {
    pub session_identity: [u8; 32],
    pub step: u64,
    pub prior_generation: u64,
    pub generation: u64,
    pub batch_identity: [u8; 32],
    pub metrics: Vec<TrainingMetric>,
    pub consumed_work_units: u64,
    pub realization_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrainStepOutcome {
    Committed(Box<CommittedTrainingStep>),
    NotCommitted {
        failure: TrainStepFailure,
        retained_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedTrainingStep {
    pub state: TrainingState,
    pub receipt: TrainStepReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationReceipt {
    pub session_identity: [u8; 32],
    pub batch_identity: [u8; 32],
    pub state_identity: String,
    pub state_generation: u64,
    pub metrics: Vec<TrainingMetric>,
    pub consumed_work_units: u64,
    pub realization_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingCheckpointReceipt {
    pub session_identity: [u8; 32],
    pub session_descriptor_identity: [u8; 32],
    pub base_artifact_identity: [u8; 32],
    pub dataset_manifest_identity: [u8; 32],
    pub split_membership_identity: [u8; 32],
    pub objective_profile: String,
    pub randomness: RandomnessProfile,
    pub realization: HostTrainingRealization,
    pub completed_steps: u64,
    pub consumed_work_units: u64,
    pub metric_summaries: Vec<TrainingMetric>,
    pub checkpoint: ModelCheckpoint,
}
