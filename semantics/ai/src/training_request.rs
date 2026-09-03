//! Complete inputs to one bounded training lifecycle transition.

use alloc::vec::Vec;
use conduit_data::{DatasetDescriptor, DatasetSplitMembership};

use super::{
    HostStepTerminal, HostTrainingRealization, TrainStepRequest, TrainingBatch, TrainingMetric,
    TrainingState,
};
use crate::{ModelArtifact, ModelCheckpoint};

pub struct TrainStepCommit<'a> {
    pub artifact: &'a ModelArtifact,
    pub dataset: &'a DatasetDescriptor,
    pub split: &'a DatasetSplitMembership,
    pub state: &'a TrainingState,
    pub request: &'a TrainStepRequest,
    pub terminal: HostStepTerminal,
    pub realization: &'a HostTrainingRealization,
}

pub struct EvaluationRequest<'a> {
    pub artifact: &'a ModelArtifact,
    pub dataset: &'a DatasetDescriptor,
    pub split: &'a DatasetSplitMembership,
    pub state: &'a TrainingState,
    pub batch: &'a TrainingBatch,
    pub metrics: Vec<TrainingMetric>,
    pub consumed_work_units: u64,
    pub realization: &'a HostTrainingRealization,
}

pub struct CheckpointRequest<'a> {
    pub artifact: &'a ModelArtifact,
    pub dataset: &'a DatasetDescriptor,
    pub split: &'a DatasetSplitMembership,
    pub state: &'a TrainingState,
    pub checkpoint: ModelCheckpoint,
    pub metric_summaries: Vec<TrainingMetric>,
    pub realization: &'a HostTrainingRealization,
}
