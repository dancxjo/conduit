//! Atomic model/optimizer updates under the existing semantic state boundary.
use burn::{module::Module, optim::ModuleOptimizer, tensor::Device};
use conduit_ai::*;
use conduit_data::{DatasetDescriptor, DatasetSplitMembership, TensorElement, TensorValue};
use sha2::{Digest, Sha256};

use crate::model::validate_values;
use crate::Cancellation;
use crate::{AuthoringDescriptor, BurnModelDefinition, DeviceRequest, Error, OptimizerRecipe};

#[derive(Debug, Clone)]
pub struct TrainingContext {
    pub artifact: ModelArtifact,
    pub dataset: DatasetDescriptor,
    pub split: DatasetSplitMembership,
    pub session: TrainingSession,
    /// Supplied by host preparation from the actual executable/build/device evidence.
    pub realization: HostTrainingRealization,
}

pub struct BurnAdapter<D: BurnModelDefinition> {
    pub(crate) definition: D,
    pub(crate) descriptor: AuthoringDescriptor,
    pub(crate) device: Device,
    pub(crate) recipe: OptimizerRecipe,
    pub(crate) context: TrainingContext,
    pub(crate) model: Option<D::Model>,
    pub(crate) optimizer: ModuleOptimizer,
    pub(crate) state: TrainingState,
    pub(crate) corpus: Option<crate::PreparedTrainingCorpus>,
    pub(crate) session: ModelComputeSession,
    pub(crate) offer: ModelComputeOffer,
    pub(crate) lifecycle: TrainingLifecycle,
    pub(crate) inference_only: bool,
    pub(crate) inference_generation: Option<u64>,
    pub(crate) inference_checkpoint_identity: Option<[u8; 32]>,
}

impl<D: BurnModelDefinition> BurnAdapter<D> {
    pub fn initialize(
        definition: D,
        request: DeviceRequest,
        recipe: OptimizerRecipe,
        context: TrainingContext,
    ) -> Result<Self, Error> {
        Self::from_prepared(
            crate::PreparedBurnModel::initialize(definition, request, recipe)?,
            context,
        )
    }
    pub fn from_prepared(
        prepared: crate::PreparedBurnModel<D>,
        context: TrainingContext,
    ) -> Result<Self, Error> {
        let crate::PreparedBurnModel {
            definition,
            descriptor,
            device,
            recipe,
            request,
            model,
            weights,
        } = prepared;
        if context.artifact.content_identity() != Sha256::digest(&weights).as_slice()
            || context.artifact.content.extent.bytes != weights.len() as u64
            || context.session.base_checkpoint_identity.is_some()
        {
            return Err(Error::IncompatibleCheckpoint);
        }
        context
            .artifact
            .validate(&descriptor.signature)
            .map_err(|_| Error::InvalidDescriptor)?;
        context
            .session
            .validate(&context.artifact, &context.dataset, &context.split)?;
        if context.artifact.architecture_profile != descriptor.architecture
            || context.artifact.state_schema_version != descriptor.checkpoint_schema
            || context.artifact.precision_profile != "number/ieee754-f32-le"
            || context.artifact.format_profile != "model/burn-safetensors@1"
            || context.realization.runtime_name != "Burn"
            || context.realization.runtime_version != "0.22.0"
            || context.realization.device_profile != request.evidence()
            || context.realization.precision_profile != context.artifact.precision_profile
            || context.realization.format_profile != context.artifact.format_profile
            || context.session.resources.model_bytes() < descriptor.resources.model_bytes
            || context.session.resources.working_memory_bytes()
                < descriptor.resources.working_bytes()?
            || context.session.resources.maximum_checkpoint_bytes()
                < descriptor.resources.checkpoint_bytes
        {
            return Err(Error::InvalidDescriptor);
        }
        let mut limits = descriptor.limits;
        limits.compute.class = match request {
            DeviceRequest::Cpu => PortableComputeClass::GeneralCpu,
            DeviceRequest::Cuda(_) => PortableComputeClass::Accelerator,
        };
        let operations = descriptor.signature.operations.get().as_slice();
        let mut supported_operations = vec![ModelComputeOperation::Checkpoint];
        if operations.contains(&ModelOperation::Infer) {
            supported_operations.push(ModelComputeOperation::Inference);
        }
        if operations.contains(&ModelOperation::Evaluate) {
            supported_operations.push(ModelComputeOperation::Evaluate);
        }
        if operations.contains(&ModelOperation::Train)
            && descriptor.groups.iter().any(|g| g.trainable)
        {
            supported_operations.push(ModelComputeOperation::TrainStep);
        }
        let offer = ModelComputeOffer {
            identity: format!(
                "std/burn/{}/{}",
                descriptor.architecture,
                request.evidence()
            ),
            supported_operations,
            accepted_formats: vec![context.artifact.format_profile.clone()],
            supported_elements: vec![TensorElement::F32],
            solver_profiles: vec![],
            determinism_profiles: vec![context.realization.deterministic_profile.clone()],
            checkpoint_loading: true,
            checkpoint_writing: true,
            limits,
            cache_policy: ModelCachePolicy::bounded(limits.maximum_model_bytes, 1)
                .map_err(|_| Error::InvalidDescriptor)?,
        };
        offer.validate()?;
        let runtime = ModelComputeRuntimeIdentity {
            provider_name: "conduit-burn".into(),
            runtime_name: "Burn".into(),
            runtime_version: "0.22.0".into(),
            runtime_build_identity: context.realization.runtime_build_identity.clone(),
            adapter_artifact_identity: context.realization.implementation_identity.clone(),
            device_evidence: context.realization.device_profile.clone(),
            precision_profile: context.realization.precision_profile.clone(),
        };
        let mut session = ModelComputeSession::discovered(offer.clone(), runtime)?;
        session.begin_load(
            context.artifact.content_identity(),
            descriptor.resources.model_bytes,
        )?;
        session.begin_warming()?;
        session.ready()?;
        let state = TrainingState {
            session_identity: context.session.identity,
            model: MutableModelState {
                base_artifact_identity: context.artifact.content_identity(),
                state_identity: format!("burn/training/{}", hex(&context.session.identity)),
                state_schema_version: descriptor.checkpoint_schema,
                generation: 0,
            },
            initial_generation: 0,
            completed_steps: 0,
            consumed_work_units: 0,
        };
        let mut lifecycle = TrainingLifecycle {
            session_identity: context.session.identity,
            phase: TrainingLifecyclePhase::Unloaded,
        };
        lifecycle.transition(TrainingLifecyclePhase::Loading)?;
        lifecycle.transition(TrainingLifecyclePhase::Ready)?;
        let optimizer = recipe.optimizer()?;
        Ok(Self {
            definition,
            descriptor,
            device,
            recipe,
            context,
            model: Some(model),
            optimizer,
            state,
            corpus: None,
            session,
            offer,
            lifecycle,
            inference_only: false,
            inference_generation: None,
            inference_checkpoint_identity: None,
        })
    }
    /// Drop all training context and optimizer state at the inference off-ramp.
    pub fn into_inference(mut self) -> Result<crate::InferenceBurnAdapter<D>, Error> {
        self.ready()?;
        if !self.inference_only {
            return Err(Error::IncompatibleCheckpoint);
        }
        let identity = self
            .inference_checkpoint_identity
            .ok_or(Error::IncompatibleCheckpoint)?;
        let runtime = self.runtime().clone();
        crate::InferenceBurnAdapter::from_loaded(
            self.definition,
            self.descriptor,
            self.device,
            self.model.take().ok_or(Error::Unloaded)?,
            crate::InferenceContext {
                artifact: self.context.artifact,
                runtime,
                determinism_profile: self.context.realization.deterministic_profile,
            },
            identity,
        )
    }
    pub fn offer(&self) -> &ModelComputeOffer {
        &self.offer
    }
    pub fn runtime(&self) -> &ModelComputeRuntimeIdentity {
        self.session.runtime()
    }
    pub fn loaded_artifact_identity(&self) -> [u8; 32] {
        self.context.artifact.content_identity()
    }
    pub fn model_bytes(&self) -> u64 {
        self.descriptor.resources.model_bytes
    }
    pub fn inference_checkpoint_identity(&self) -> Option<[u8; 32]> {
        self.inference_checkpoint_identity
    }
    pub fn signature(&self) -> &ModelSignature {
        &self.descriptor.signature
    }
    pub fn admitted_inference_work(&self) -> u64 {
        self.descriptor.resources.maximum_work_per_step
    }
    pub fn admit_inference_requirement(
        &self,
        requirement: &ModelComputeRequirement,
    ) -> Result<(), Error> {
        self.offer.admits(requirement)?;
        let needed = self.requirement(
            ModelComputeOperation::Inference,
            requirement.input_bytes,
            self.offer.limits.maximum_output_bytes,
        );
        if requirement.operation != ModelComputeOperation::Inference
            || requirement.model_bytes < needed.model_bytes
            || requirement.working_memory_bytes < needed.working_memory_bytes
            || requirement.device_memory_bytes < needed.device_memory_bytes
            || requirement.output_bytes < needed.output_bytes
        {
            return Err(Error::ResourceBound);
        }
        Ok(())
    }
    pub fn ready_for_inference(&self) -> Result<(), Error> {
        self.ready()
    }
    pub fn is_inference_only(&self) -> bool {
        self.inference_only
    }
    pub fn state(&self) -> &TrainingState {
        &self.state
    }
    /// Complete retained semantic context for Host admission and receipt bounds.
    pub fn training_context(&self) -> &TrainingContext {
        &self.context
    }
    pub fn lifecycle(&self) -> &TrainingLifecycle {
        &self.lifecycle
    }

    pub(crate) fn requirement(
        &self,
        operation: ModelComputeOperation,
        input_bytes: u64,
        output_bytes: u64,
    ) -> ModelComputeRequirement {
        let limits = self.offer.limits;
        ModelComputeRequirement {
            operation,
            model_format: self.context.artifact.format_profile.clone(),
            element: TensorElement::F32,
            rank: limits.maximum_rank,
            model_bytes: self.descriptor.resources.model_bytes,
            working_memory_bytes: self
                .descriptor
                .resources
                .working_bytes()
                .and_then(|base| {
                    base.checked_add(
                        self.corpus
                            .as_ref()
                            .map_or(Ok(0), crate::PreparedTrainingCorpus::working_bytes)?,
                    )
                    .ok_or(Error::ResourceBound)
                })
                .unwrap_or(u64::MAX),
            device_memory_bytes: if limits.compute.class == PortableComputeClass::Accelerator {
                self.descriptor.resources.model_bytes.saturating_add(
                    self.descriptor
                        .resources
                        .working_bytes()
                        .unwrap_or(u64::MAX),
                )
            } else {
                0
            },
            input_bytes: input_bytes.max(1),
            output_bytes: output_bytes.max(1),
            batch_items: limits.maximum_batch_items,
            compute_class: limits.compute.class,
            minimum_lanes: limits.compute.minimum_lanes,
            preferred_lanes: limits.compute.preferred_lanes,
            maximum_lanes: limits.compute.maximum_lanes,
            minimum_service: limits.compute.service,
            solver_profile: None,
            determinism_profile: self.context.realization.deterministic_profile.clone(),
            requires_checkpoint_load: false,
            requires_checkpoint_write: operation == ModelComputeOperation::Checkpoint,
        }
    }
    pub fn infer(&mut self, inputs: &[TensorValue]) -> Result<Vec<TensorValue>, Error> {
        self.infer_cancellable(inputs, &Cancellation::default())
    }
    pub fn infer_cancellable(
        &mut self,
        inputs: &[TensorValue],
        cancel: &Cancellation,
    ) -> Result<Vec<TensorValue>, Error> {
        self.ready()?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let bytes = validate_values(
            inputs,
            self.descriptor.signature.inputs.get().as_slice(),
            self.offer.limits.maximum_input_bytes,
        )?;
        let requirement = self.requirement(
            ModelComputeOperation::Inference,
            bytes,
            self.offer.limits.maximum_output_bytes,
        );
        self.session.begin(&requirement, 0)?;
        let model = self
            .model
            .as_ref()
            .ok_or(Error::Unloaded)?
            .clone()
            .fork(&self.device)
            .valid();
        let result = self
            .definition
            .forward(&model, inputs, &self.device)
            .and_then(|outputs| {
                validate_values(
                    &outputs,
                    self.descriptor.signature.outputs.get().as_slice(),
                    self.offer.limits.maximum_output_bytes,
                )?;
                Ok(outputs)
            });
        if cancel.is_cancelled() {
            self.session.cancel()?;
            return Err(Error::Cancelled);
        }
        self.session.finish()?;
        let _publication = cancel.commit_guard()?;
        result
    }
    pub fn snapshot_identity(&self) -> Result<[u8; 32], Error> {
        let model = self
            .model
            .as_ref()
            .ok_or(Error::Unloaded)?
            .clone()
            .into_record()
            .into_bytes()
            .map_err(|_| Error::NumericFailure)?;
        let optimizer = self
            .optimizer
            .into_bytes()
            .map_err(|_| Error::NumericFailure)?;
        let mut hash = Sha256::new();
        hash.update(&*model);
        hash.update(&*optimizer);
        Ok(hash.finalize().into())
    }
    pub fn unload(&mut self) -> Result<(), Error> {
        self.session.begin_unload()?;
        self.session.shutdown()?;
        self.lifecycle
            .transition(TrainingLifecyclePhase::Unloaded)?;
        self.model = None;
        self.corpus = None;
        self.optimizer = self.recipe.optimizer()?;
        Ok(())
    }
    pub(crate) fn ready(&self) -> Result<(), Error> {
        if self.model.is_none() {
            return Err(Error::Unloaded);
        }
        if self.lifecycle.phase != TrainingLifecyclePhase::Ready {
            return Err(Error::ModelCompute(
                ModelComputeRefusal::InvalidLifecycleTransition,
            ));
        }
        Ok(())
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
