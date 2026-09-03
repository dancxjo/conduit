//! Stable identities for training intent, work, and durable receipts.

use alloc::vec::Vec;
use conduit_core::semantic_digest;
use conduit_data::{DatasetDescriptor, DatasetSplitMembership};

use crate::{
    BatchOrder, CheckpointPolicy, EvaluationPolicy, EvaluationReceipt, MissingModalityPolicy,
    ObjectiveParticipation, RandomnessProfile, TrainStepReceipt, TrainingBatch,
    TrainingCheckpointReceipt, TrainingRefusal, TrainingSession,
};

impl TrainingSession {
    pub fn semantic_digest(
        &self,
        artifact: &crate::ModelArtifact,
        dataset: &DatasetDescriptor,
        split: &DatasetSplitMembership,
    ) -> Result<[u8; 32], TrainingRefusal> {
        self.validate(artifact, dataset, split)?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.identity);
        bytes.extend_from_slice(&self.base_artifact_identity);
        push_optional_digest(&mut bytes, self.base_checkpoint_identity);
        bytes.extend_from_slice(&self.dataset_manifest_identity);
        bytes.extend_from_slice(&self.split_membership_identity);
        push_text(&mut bytes, &self.objective_profile);
        push_len(&mut bytes, self.objectives.len());
        for objective in &self.objectives {
            push_text(&mut bytes, &objective.role);
            bytes.extend_from_slice(&objective.weight_millionths.to_le_bytes());
            push_text(&mut bytes, &objective.configuration_identity);
            push_text(&mut bytes, &objective.output_identity);
            bytes.push(match objective.participation {
                ObjectiveParticipation::Optimize => 0,
                ObjectiveParticipation::ObserveOnly => 1,
            });
        }
        push_randomness(&mut bytes, &self.randomness);
        push_text(&mut bytes, &self.precision_profile);
        push_len(&mut bytes, self.model_modalities.len());
        for modality in &self.model_modalities {
            push_text(&mut bytes, modality);
        }
        match &self.missing_modality_policy {
            MissingModalityPolicy::Reject => bytes.push(0),
            MissingModalityPolicy::PermitDeclared {
                optional_modalities,
            } => {
                bytes.push(1);
                push_len(&mut bytes, optional_modalities.len());
                for modality in optional_modalities {
                    push_text(&mut bytes, modality);
                }
            }
        }
        let resources = self.resources;
        bytes.extend_from_slice(&resources.model_bytes.to_le_bytes());
        bytes.extend_from_slice(&resources.working_memory_bytes.to_le_bytes());
        bytes.extend_from_slice(&resources.compute_lanes.to_le_bytes());
        bytes.extend_from_slice(&resources.maximum_batch_items.to_le_bytes());
        bytes.extend_from_slice(&resources.maximum_batch_bytes.to_le_bytes());
        bytes.extend_from_slice(&resources.maximum_steps.to_le_bytes());
        bytes.extend_from_slice(&resources.maximum_work_units.to_le_bytes());
        bytes.extend_from_slice(&resources.maximum_checkpoint_bytes.to_le_bytes());
        bytes.extend_from_slice(&resources.maximum_in_flight_steps.to_le_bytes());
        push_checkpoint_policy(&mut bytes, self.checkpoint_policy);
        push_evaluation_policy(&mut bytes, self.evaluation_policy);
        Ok(semantic_digest("ai/training-session@1", &bytes))
    }
}

impl TrainingBatch {
    pub fn semantic_digest(
        &self,
        session: &TrainingSession,
        split: &DatasetSplitMembership,
    ) -> Result<[u8; 32], TrainingRefusal> {
        self.validate_for(session, split)?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.identity);
        bytes.extend_from_slice(&self.dataset_identity);
        push_text(&mut bytes, &self.split_identity);
        push_digests(&mut bytes, &self.example_identities);
        push_len(&mut bytes, self.present_modalities.len());
        for modality in &self.present_modalities {
            push_text(&mut bytes, modality);
        }
        bytes.extend_from_slice(&self.encoded_bytes.to_le_bytes());
        bytes.push(match self.order {
            BatchOrder::Stable => 0,
            BatchOrder::Shuffled => 1,
        });
        match self.stochastic_seed {
            None => bytes.push(0),
            Some(seed) => {
                bytes.push(1);
                bytes.extend_from_slice(&seed.to_le_bytes());
            }
        }
        Ok(semantic_digest("ai/training-batch@1", &bytes))
    }
}

impl TrainStepReceipt {
    pub fn semantic_digest(&self) -> [u8; 32] {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.session_identity);
        bytes.extend_from_slice(&self.step.to_le_bytes());
        bytes.extend_from_slice(&self.prior_generation.to_le_bytes());
        bytes.extend_from_slice(&self.generation.to_le_bytes());
        bytes.extend_from_slice(&self.batch_identity);
        push_metrics(&mut bytes, &self.metrics);
        bytes.extend_from_slice(&self.consumed_work_units.to_le_bytes());
        push_text(&mut bytes, &self.realization_identity);
        semantic_digest("ai/train-step-receipt@1", &bytes)
    }
}

impl EvaluationReceipt {
    pub fn semantic_digest(&self) -> [u8; 32] {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.session_identity);
        bytes.extend_from_slice(&self.batch_identity);
        push_text(&mut bytes, &self.state_identity);
        bytes.extend_from_slice(&self.state_generation.to_le_bytes());
        push_metrics(&mut bytes, &self.metrics);
        bytes.extend_from_slice(&self.consumed_work_units.to_le_bytes());
        push_text(&mut bytes, &self.realization_identity);
        semantic_digest("ai/evaluation-receipt@1", &bytes)
    }
}

impl TrainingCheckpointReceipt {
    pub fn semantic_digest(&self) -> [u8; 32] {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.session_identity);
        bytes.extend_from_slice(&self.base_artifact_identity);
        bytes.extend_from_slice(&self.dataset_manifest_identity);
        bytes.extend_from_slice(&self.split_membership_identity);
        push_text(&mut bytes, &self.objective_profile);
        push_randomness(&mut bytes, &self.randomness);
        bytes.extend_from_slice(&self.session_descriptor_identity);
        push_text(&mut bytes, &self.realization.implementation_identity);
        push_text(&mut bytes, &self.realization.runtime_name);
        push_text(&mut bytes, &self.realization.runtime_version);
        push_text(&mut bytes, &self.realization.runtime_build_identity);
        push_text(&mut bytes, &self.realization.device_profile);
        push_text(&mut bytes, &self.realization.format_profile);
        push_text(&mut bytes, &self.realization.precision_profile);
        push_text(&mut bytes, &self.realization.deterministic_profile);
        bytes.extend_from_slice(&self.completed_steps.to_le_bytes());
        bytes.extend_from_slice(&self.consumed_work_units.to_le_bytes());
        push_metrics(&mut bytes, &self.metric_summaries);
        bytes.extend_from_slice(&self.checkpoint.content.identity.digest());
        bytes.extend_from_slice(&self.checkpoint.generation.to_le_bytes());
        semantic_digest("ai/training-checkpoint-receipt@1", &bytes)
    }
}

fn push_metrics(output: &mut Vec<u8>, values: &[crate::TrainingMetric]) {
    push_len(output, values.len());
    for value in values {
        push_text(output, &value.output_identity);
        output.extend_from_slice(&value.value_millionths.to_le_bytes());
    }
}

fn push_randomness(output: &mut Vec<u8>, value: &RandomnessProfile) {
    match value {
        RandomnessProfile::Deterministic => output.push(0),
        RandomnessProfile::ExplicitSeed(seed) => {
            output.push(1);
            output.extend_from_slice(&seed.to_le_bytes());
        }
        RandomnessProfile::ProviderChosen { seed, nonce } => {
            output.push(2);
            output.extend_from_slice(&seed.to_le_bytes());
            push_text(output, nonce);
        }
    }
}

fn push_checkpoint_policy(output: &mut Vec<u8>, value: CheckpointPolicy) {
    match value {
        CheckpointPolicy::None => output.push(0),
        CheckpointPolicy::EverySteps(steps) => {
            output.push(1);
            output.extend_from_slice(&steps.to_le_bytes());
        }
        CheckpointPolicy::AtCompletion => output.push(2),
    }
}

fn push_evaluation_policy(output: &mut Vec<u8>, value: EvaluationPolicy) {
    match value {
        EvaluationPolicy::None => output.push(0),
        EvaluationPolicy::EverySteps(steps) => {
            output.push(1);
            output.extend_from_slice(&steps.to_le_bytes());
        }
        EvaluationPolicy::AtCompletion => output.push(2),
    }
}

fn push_optional_digest(output: &mut Vec<u8>, value: Option<[u8; 32]>) {
    match value {
        None => output.push(0),
        Some(value) => {
            output.push(1);
            output.extend_from_slice(&value);
        }
    }
}

fn push_digests(output: &mut Vec<u8>, values: &[[u8; 32]]) {
    push_len(output, values.len());
    for value in values {
        output.extend_from_slice(value);
    }
}

fn push_len(output: &mut Vec<u8>, value: usize) {
    output.extend_from_slice(&(value as u16).to_le_bytes());
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u16).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}
