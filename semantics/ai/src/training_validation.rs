use alloc::{string::String, vec::Vec};
use conduit_data::DatasetSplitMembership;

use super::*;
use crate::{
    ModelArtifact, ModelRuntimeRealization, MutableModelState, TrainStepRequest, TrainingBatch,
    TrainingSession,
};

impl TrainingState {
    pub(super) fn validate_for(
        &self,
        session: &TrainingSession,
        artifact: &ModelArtifact,
    ) -> Result<(), TrainingRefusal> {
        if self.session_identity != session.identity
            || self.completed_steps > session.resources.maximum_steps()
            || self.consumed_work_units > session.resources.maximum_work_units()
            || self.model.generation
                != self
                    .initial_generation
                    .checked_add(self.completed_steps)
                    .ok_or(TrainingRefusal::StaleState)?
        {
            return Err(TrainingRefusal::StaleState);
        }
        self.model
            .validate(artifact)
            .map_err(|_| TrainingRefusal::StaleState)
    }
}

impl TrainingBatch {
    pub(crate) fn validate_for(
        &self,
        session: &TrainingSession,
        split: &DatasetSplitMembership,
    ) -> Result<(), TrainingRefusal> {
        nonzero(self.identity)?;
        nonzero(self.dataset_identity)?;
        text(&self.split_identity)?;
        let example_identities = self.example_identities_iter().collect::<Vec<_>>();
        let present_modalities = self.present_modality_strings().collect::<Vec<_>>();
        if self.example_identities.get().iter().any(|page| {
            page.get().as_slice().is_empty()
                || page.get().as_slice().len() % core::mem::size_of::<[u8; 32]>() != 0
        }) || self.dataset_identity != split.dataset_identity
            || self.split_identity != split.split_identity
            || example_identities.is_empty()
            || example_identities
                .iter()
                .any(|item| !split.identities().any(|identity| *identity.get() == **item))
            || example_identities.contains(&&[0; 32])
            || example_identities
                .iter()
                .enumerate()
                .any(|(index, value)| example_identities[index + 1..].contains(value))
        {
            return Err(TrainingRefusal::InvalidBatch);
        }
        if example_identities.len() > session.resources.maximum_batch_items() as usize
            || self.encoded_bytes == 0
            || self.encoded_bytes > session.resources.maximum_batch_bytes()
            || present_modalities.is_empty()
            || present_modalities.len() > MAXIMUM_BATCH_MODALITIES
            || present_modalities
                .iter()
                .enumerate()
                .any(|(index, value)| present_modalities[index + 1..].contains(value))
        {
            return Err(TrainingRefusal::BatchBoundExceeded);
        }
        for modality in &present_modalities {
            text(modality)?;
            if !session
                .model_modality_strings()
                .any(|candidate| candidate == *modality)
            {
                return Err(TrainingRefusal::InvalidBatch);
            }
        }
        match self.order {
            BatchOrder::Stable if self.stochastic_seed.is_some() => {
                return Err(TrainingRefusal::UnexpectedSeed)
            }
            BatchOrder::Shuffled if self.stochastic_seed.is_none() => {
                return Err(TrainingRefusal::UnexpectedSeed)
            }
            _ => {}
        }
        for modality in session.model_modality_strings() {
            if present_modalities.contains(&modality) {
                continue;
            }
            match &session.missing_modality_policy {
                MissingModalityPolicy::Reject => {
                    return Err(TrainingRefusal::MissingRequiredModality)
                }
                MissingModalityPolicy::PermitDeclared(policy)
                    if policy
                        .optional_modalities()
                        .iter()
                        .any(|candidate| candidate.get() == modality) => {}
                MissingModalityPolicy::PermitDeclared(_) => {
                    return Err(TrainingRefusal::MissingRequiredModality)
                }
            }
        }
        Ok(())
    }
}

impl TrainStepRequest {
    pub(super) fn validate_for(
        &self,
        session: &TrainingSession,
        split: &DatasetSplitMembership,
    ) -> Result<(), TrainingRefusal> {
        if self.step == 0 || self.step > session.resources.maximum_steps() {
            return Err(TrainingRefusal::StepBoundExceeded);
        }
        if self.admitted_work_units == 0
            || self.admitted_work_units > session.resources.maximum_work_units()
        {
            return Err(TrainingRefusal::WorkBoundExceeded);
        }
        self.batch.validate_for(session, split)
    }
}

impl HostStepCandidate {
    pub(super) fn validate_for(
        &self,
        session: &TrainingSession,
        state: &MutableModelState,
        request: &TrainStepRequest,
    ) -> Result<(), TrainingRefusal> {
        text(&self.state_identity)?;
        if self.state_identity != state.state_identity
            || self.state_schema_version != state.state_schema_version
            || self.generation
                != state
                    .generation
                    .checked_add(1)
                    .ok_or(TrainingRefusal::InvalidCandidate)?
        {
            return Err(TrainingRefusal::InvalidCandidate);
        }
        validate_metrics(&self.metrics, session.objectives_slice())?;
        if self.consumed_work_units == 0 || self.consumed_work_units > request.admitted_work_units {
            return Err(TrainingRefusal::WorkBoundExceeded);
        }
        Ok(())
    }
}

impl HostTrainingRealization {
    pub(super) fn validate_for(
        &self,
        session: &TrainingSession,
        artifact: &ModelArtifact,
    ) -> Result<(), TrainingRefusal> {
        for value in [
            &self.implementation_identity,
            &self.runtime_name,
            &self.runtime_version,
            &self.runtime_build_identity,
            &self.device_profile,
            &self.format_profile,
            &self.precision_profile,
            &self.deterministic_profile,
        ] {
            text(value)?;
        }
        if self.format_profile != artifact.format_profile
            || self.precision_profile != session.precision_profile
            || self.precision_profile != artifact.precision_profile
        {
            return Err(TrainingRefusal::InvalidRealization);
        }
        Ok(())
    }

    pub fn as_model_runtime(&self, artifact: &ModelArtifact) -> ModelRuntimeRealization {
        ModelRuntimeRealization {
            implementation_identity: self.implementation_identity.clone(),
            runtime_name: self.runtime_name.clone(),
            runtime_version: self.runtime_version.clone(),
            runtime_build_identity: self.runtime_build_identity.clone(),
            device_profile: self.device_profile.clone(),
            supported_formats: alloc::vec![self.format_profile.clone()],
            supported_precisions: alloc::vec![self.precision_profile.clone()],
            loaded_artifact_identity: artifact.content_identity(),
            loaded_checkpoint_identity: None,
        }
    }
}

pub(super) fn validate_objectives(values: &[TrainingObjective]) -> Result<(), TrainingRefusal> {
    if values.is_empty() {
        return Err(TrainingRefusal::InvalidObjective);
    }
    if values.len() > MAXIMUM_TRAINING_OBJECTIVES {
        return Err(TrainingRefusal::TooManyObjectives);
    }
    let mut optimization = false;
    for value in values {
        if value.participation() == &ObjectiveParticipation::Optimize {
            optimization = true;
        }
    }
    let outputs = values
        .iter()
        .map(|value| value.output_identity().get().clone())
        .collect::<Vec<_>>();
    if has_duplicate_text(&outputs) {
        return Err(TrainingRefusal::InvalidObjective);
    }
    if !optimization {
        return Err(TrainingRefusal::NoOptimizationObjective);
    }
    Ok(())
}

pub(super) fn validate_metrics(
    values: &[TrainingMetric],
    objectives: &[TrainingObjective],
) -> Result<(), TrainingRefusal> {
    if values.is_empty() || values.len() > MAXIMUM_METRICS {
        return Err(TrainingRefusal::InvalidMetric);
    }
    for value in values {
        if !objectives
            .iter()
            .any(|objective| objective.output_identity() == value.output_identity())
        {
            return Err(TrainingRefusal::InvalidMetric);
        }
    }
    let outputs = values
        .iter()
        .map(|value| value.output_identity().get().clone())
        .collect::<Vec<_>>();
    if has_duplicate_text(&outputs) {
        return Err(TrainingRefusal::DuplicateMetric);
    }
    Ok(())
}

pub(super) fn validate_interval_policy(policy: CheckpointPolicy) -> Result<(), TrainingRefusal> {
    if matches!(policy, CheckpointPolicy::EverySteps(0)) {
        Err(TrainingRefusal::InvalidPolicy)
    } else {
        Ok(())
    }
}

pub(super) fn validate_evaluation_policy(policy: EvaluationPolicy) -> Result<(), TrainingRefusal> {
    if matches!(policy, EvaluationPolicy::EverySteps(0)) {
        Err(TrainingRefusal::InvalidPolicy)
    } else {
        Ok(())
    }
}

pub(super) fn text(value: &str) -> Result<(), TrainingRefusal> {
    if value.is_empty() || value.len() > 128 {
        Err(TrainingRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
}

pub(super) fn nonzero(value: [u8; 32]) -> Result<(), TrainingRefusal> {
    if value == [0; 32] {
        Err(TrainingRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
}

pub(super) fn has_duplicate_text(values: &[String]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}
