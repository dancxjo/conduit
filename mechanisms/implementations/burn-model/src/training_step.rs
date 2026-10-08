//! Candidate autodiff work and publication through the semantic training boundary.
use crate::model::validate_values;
use crate::{BurnAdapter, BurnModelDefinition, Cancellation, Error, ModelBatch};
use burn::{
    module::Module,
    optim::{GradientsParams, ModuleOptimizer},
};
use conduit_ai::*;

impl<D: BurnModelDefinition> BurnAdapter<D> {
    pub fn evaluate(
        &self,
        batch_identity: &TrainingBatch,
        batch: &ModelBatch,
    ) -> Result<EvaluationReceipt, Error> {
        self.ready()?;
        if self.inference_only {
            return Err(Error::IncompatibleCheckpoint);
        }
        self.validate_batch(batch, batch_identity)?;
        self.offer.admits(&self.requirement(
            ModelComputeOperation::Evaluate,
            batch_identity.encoded_bytes,
            self.offer.limits.maximum_output_bytes,
        ))?;
        let model = self
            .model
            .as_ref()
            .ok_or(Error::Unloaded)?
            .clone()
            .fork(&self.device)
            .valid();
        let objective = self.definition.objective(
            &model,
            batch,
            self.context.session.objectives_slice(),
            &self.device,
        )?;
        let loss: f32 = objective.loss.into_scalar();
        if !loss.is_finite() {
            return Err(Error::NumericFailure);
        }
        Ok(self.context.session.evaluate(EvaluationRequest {
            artifact: &self.context.artifact,
            dataset: &self.context.dataset,
            split: &self.context.split,
            state: &self.state,
            batch: batch_identity,
            metrics: objective.metrics,
            consumed_work_units: self.descriptor.resources.maximum_work_per_step,
            realization: &self.context.realization,
        })?)
    }
    pub fn train_step(
        &mut self,
        request: &TrainStepRequest,
        batch: &ModelBatch,
        cancel: &Cancellation,
    ) -> Result<TrainStepOutcome, Error> {
        self.ready()?;
        if self.inference_only {
            return Err(Error::IncompatibleCheckpoint);
        }
        self.validate_batch(batch, &request.batch)?;
        // Use the authoritative semantic validation before entering Burn or changing RNG.
        let preflight = self.semantic_commit(
            request,
            HostStepTerminal::NoCommit(TrainStepFailure::Cancelled),
        )?;
        if request.admitted_work_units < self.descriptor.resources.maximum_work_per_step {
            return Err(Error::ResourceBound);
        }
        if cancel.is_cancelled() {
            return Ok(preflight);
        }
        let requirement = self.requirement(
            ModelComputeOperation::TrainStep,
            request.batch.encoded_bytes,
            self.offer.limits.maximum_output_bytes,
        );
        self.session.begin(&requirement, 0)?;
        self.lifecycle.transition(
            TrainingLifecyclePhase::active_step(request.step)
                .map_err(|_| Error::InvalidDescriptor)?,
        )?;
        self.device
            .seed(self.recipe.seed.wrapping_add(request.step));
        let result = self.prepare_step(request, batch, cancel);
        self.session.finish()?;
        match result {
            Ok((model, optimizer, outcome)) => {
                if let TrainStepOutcome::Committed(committed) = &outcome {
                    self.model = Some(model);
                    self.optimizer = optimizer;
                    self.state = committed.state.clone();
                    self.inference_checkpoint_identity = None;
                    self.inference_generation = None;
                }
                self.lifecycle.transition(TrainingLifecyclePhase::Ready)?;
                Ok(outcome)
            }
            Err(Error::Cancelled) => {
                self.lifecycle
                    .transition(TrainingLifecyclePhase::Cancelled)?;
                self.semantic_commit(
                    request,
                    HostStepTerminal::NoCommit(TrainStepFailure::Cancelled),
                )
            }
            Err(error) => {
                self.lifecycle.transition(TrainingLifecyclePhase::Failed)?;
                let failure = match error {
                    Error::ResourceBound => TrainStepFailure::ResourceExhausted,
                    Error::Unloaded => TrainStepFailure::ProviderLost,
                    _ => TrainStepFailure::Failed,
                };
                self.semantic_commit(request, HostStepTerminal::NoCommit(failure))
            }
        }
    }
    fn prepare_step(
        &self,
        request: &TrainStepRequest,
        batch: &ModelBatch,
        cancel: &Cancellation,
    ) -> Result<(D::Model, ModuleOptimizer, TrainStepOutcome), Error> {
        let model = self
            .model
            .as_ref()
            .ok_or(Error::Unloaded)?
            .clone()
            .fork(&self.device);
        let mut optimizer = self.optimizer.clone();
        let objective = self.definition.objective(
            &model,
            batch,
            self.context.session.objectives_slice(),
            &self.device,
        )?;
        let loss: f32 = objective.loss.clone().into_scalar();
        if !loss.is_finite() {
            return Err(Error::NumericFailure);
        }
        let gradients = GradientsParams::from_grads(objective.loss.backward(), &model);
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let model = optimizer.step(self.recipe.learning_rate, model, gradients);
        // Synchronize and bound actual serialized candidate state before publication.
        let model_bytes = model
            .clone()
            .into_record()
            .into_bytes()
            .map_err(|_| Error::NumericFailure)?;
        let optimizer_bytes = optimizer.into_bytes().map_err(|_| Error::NumericFailure)?;
        if optimizer_bytes.len() as u64 > self.descriptor.resources.optimizer_bytes
            || (model_bytes.len() + optimizer_bytes.len()) as u64
                > self.descriptor.resources.candidate_bytes
        {
            return Err(Error::ResourceBound);
        }
        let model_ids = crate::record_validation::validate_record(model_bytes, false)?;
        let optimizer_ids = crate::record_validation::validate_record(optimizer_bytes, true)?;
        if !optimizer_ids.is_subset(&model_ids) {
            return Err(Error::CorruptCheckpoint);
        }
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let _publication = cancel.commit_guard()?;
        let outcome = self.semantic_commit(
            request,
            HostStepTerminal::Candidate(HostStepCandidate {
                state_identity: self.state.model.state_identity.clone(),
                state_schema_version: self.state.model.state_schema_version,
                generation: self
                    .state
                    .model
                    .generation
                    .checked_add(1)
                    .ok_or(Error::ResourceBound)?,
                metrics: objective.metrics,
                consumed_work_units: self.descriptor.resources.maximum_work_per_step,
            }),
        )?;
        Ok((model, optimizer, outcome))
    }
    fn semantic_commit(
        &self,
        request: &TrainStepRequest,
        terminal: HostStepTerminal,
    ) -> Result<TrainStepOutcome, Error> {
        Ok(self.context.session.commit_step(TrainStepCommit {
            artifact: &self.context.artifact,
            dataset: &self.context.dataset,
            split: &self.context.split,
            state: &self.state,
            request,
            terminal,
            realization: &self.context.realization,
        })?)
    }

    fn validate_batch(&self, batch: &ModelBatch, identity: &TrainingBatch) -> Result<(), Error> {
        let inputs = validate_values(
            &batch.inputs,
            self.descriptor.signature.inputs.get().as_slice(),
            self.offer.limits.maximum_input_bytes,
        )?;
        let targets = validate_values(
            &batch.targets,
            self.descriptor.signature.outputs.get().as_slice(),
            self.offer.limits.maximum_output_bytes,
        )?;
        if inputs.checked_add(targets).ok_or(Error::ResourceBound)? != identity.encoded_bytes {
            return Err(Error::InvalidTensor);
        }
        Ok(())
    }
}
