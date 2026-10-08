//! Safe inference export and compatible provider-owned resume state.
use crate::checkpoint_manifest::{Descriptor, Resume};
use crate::checkpoint_store::{digest, BlobRef};
use crate::{BurnAdapter, BurnModelDefinition, Cancellation, DirectoryCheckpointStore, Error};
use burn::{
    module::Module,
    store::{ModuleRecord, ModuleSnapshot, SafetensorsStore},
};
use conduit_ai::{
    CheckpointRequest, ModelCheckpoint, TrainingCheckpointReceipt, TrainingMetric,
    MODEL_CHECKPOINT_INFO_ID,
};
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};

impl<D: BurnModelDefinition> BurnAdapter<D> {
    pub fn checkpoint(
        &self,
        store: &DirectoryCheckpointStore,
        metrics: Vec<TrainingMetric>,
        cancel: &Cancellation,
    ) -> Result<TrainingCheckpointReceipt, Error> {
        self.ready()?;
        if self.inference_only {
            return Err(Error::IncompatibleCheckpoint);
        }
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        self.offer.admits(&self.requirement(
            conduit_ai::ModelComputeOperation::Checkpoint,
            1,
            1,
        ))?;
        let mut safetensors = SafetensorsStore::from_bytes(None).clear_metadata();
        let model = self.model.as_ref().ok_or(Error::Unloaded)?;
        model
            .save_into(&mut safetensors)
            .map_err(|_| Error::NumericFailure)?;
        let inference = safetensors.get_bytes().map_err(|_| Error::NumericFailure)?;
        let model_bytes = model
            .clone()
            .into_record()
            .into_bytes()
            .map_err(|_| Error::NumericFailure)?;
        let optimizer = self
            .optimizer
            .into_bytes()
            .map_err(|_| Error::NumericFailure)?;
        let descriptor = Descriptor {
            schema: 1,
            architecture: self.descriptor.architecture.clone(),
            config_identity: self.descriptor.config_identity,
            signature_identity: self
                .descriptor
                .signature
                .semantic_digest()
                .map_err(|_| Error::InvalidDescriptor)?,
            checkpoint_schema: self.descriptor.checkpoint_schema,
            generation: self
                .inference_generation
                .unwrap_or(self.state.model.generation),
            base_artifact_identity: self.context.artifact.content_identity(),
            group_identity: self.group_identity()?,
            inference: BlobRef::of(&inference),
            resume: Some(Resume {
                context_identity: self.context_identity()?,
                recipe: self.recipe.clone(),
                model: BlobRef::of(&model_bytes),
                optimizer: BlobRef::of(&optimizer),
                initial_generation: self.state.initial_generation,
                generation: self.state.model.generation,
                steps: self.state.completed_steps,
                work: self.state.consumed_work_units,
                state_identity: self.state.model.state_identity.clone(),
                runtime_version: "0.22.0".into(),
            }),
        };
        let bytes = serde_json::to_vec(&descriptor).map_err(|_| Error::InvalidDescriptor)?;
        let extent = bytes.len() as u64
            + inference.len() as u64
            + model_bytes.len() as u64
            + optimizer.len() as u64;
        if extent > self.descriptor.resources.checkpoint_bytes {
            return Err(Error::ResourceBound);
        }
        let checkpoint = self.checkpoint_ref(digest(&bytes), extent, descriptor.generation);
        // Validate semantic policy/cadence and receipt before durable publication.
        let receipt = self.context.session.checkpoint(CheckpointRequest {
            artifact: &self.context.artifact,
            dataset: &self.context.dataset,
            split: &self.context.split,
            state: &self.state,
            checkpoint,
            metric_summaries: metrics,
            realization: &self.context.realization,
        })?;
        store.publish(&bytes, &[&inference, &model_bytes, &optimizer], cancel)?;
        Ok(receipt)
    }
    /// An explicit off-ramp containing no optimizer or resume state.
    pub fn export_inference(
        &self,
        store: &DirectoryCheckpointStore,
        cancel: &Cancellation,
    ) -> Result<ModelCheckpoint, Error> {
        self.ready()?;
        let mut safetensors = SafetensorsStore::from_bytes(None).clear_metadata();
        self.model
            .as_ref()
            .ok_or(Error::Unloaded)?
            .save_into(&mut safetensors)
            .map_err(|_| Error::NumericFailure)?;
        let inference = safetensors.get_bytes().map_err(|_| Error::NumericFailure)?;
        let descriptor = Descriptor {
            schema: 1,
            architecture: self.descriptor.architecture.clone(),
            config_identity: self.descriptor.config_identity,
            signature_identity: self
                .descriptor
                .signature
                .semantic_digest()
                .map_err(|_| Error::InvalidDescriptor)?,
            checkpoint_schema: self.descriptor.checkpoint_schema,
            generation: self
                .inference_generation
                .unwrap_or(self.state.model.generation),
            base_artifact_identity: self.context.artifact.content_identity(),
            group_identity: self.group_identity()?,
            inference: BlobRef::of(&inference),
            resume: None,
        };
        let bytes = serde_json::to_vec(&descriptor).map_err(|_| Error::InvalidDescriptor)?;
        let extent = (bytes.len() + inference.len()) as u64;
        if extent > self.descriptor.resources.checkpoint_bytes {
            return Err(Error::ResourceBound);
        }
        let checkpoint = self.checkpoint_ref(digest(&bytes), extent, descriptor.generation);
        store.publish(&bytes, &[&inference], cancel)?;
        Ok(checkpoint)
    }
    pub fn load_inference(
        &mut self,
        store: &DirectoryCheckpointStore,
        identity: &[u8; 32],
    ) -> Result<(), Error> {
        self.ready()?;
        let descriptor = self.read_descriptor(store, identity)?;
        let bytes = store.read_blob(&descriptor.inference)?;
        let mut safetensors = SafetensorsStore::from_bytes(Some(bytes))
            .allow_partial(false)
            .validate(true);
        let mut model = self
            .model
            .as_ref()
            .ok_or(Error::Unloaded)?
            .clone()
            .fork(&self.device)
            .valid();
        model
            .load_from(&mut safetensors)
            .map_err(|_| Error::CorruptCheckpoint)?;
        let record = model
            .clone()
            .into_record()
            .into_bytes()
            .map_err(|_| Error::CorruptCheckpoint)?;
        if record.len() as u64 > self.descriptor.resources.checkpoint_bytes {
            return Err(Error::ResourceBound);
        }
        crate::record_validation::validate_record(record, false)
            .map_err(|_| Error::CorruptCheckpoint)?;
        self.model = Some(model);
        self.optimizer = self.recipe.optimizer()?;
        self.inference_only = true;
        self.inference_checkpoint_identity = Some(*identity);
        self.inference_generation = Some(descriptor.generation);
        Ok(())
    }
    pub fn resume(
        &mut self,
        store: &DirectoryCheckpointStore,
        identity: &[u8; 32],
    ) -> Result<(), Error> {
        self.ready()?;
        let descriptor = self.read_descriptor(store, identity)?;
        if self.inference_only {
            return Err(Error::IncompatibleCheckpoint);
        }
        let resume = descriptor.resume.ok_or(Error::IncompatibleCheckpoint)?;
        if resume.generation != descriptor.generation
            || resume.context_identity != self.context_identity()?
            || resume.recipe != self.recipe
            || resume.runtime_version != "0.22.0"
            || resume.state_identity != self.state.model.state_identity
            || resume.initial_generation != self.state.initial_generation
            || resume.generation < self.state.model.generation
            || resume.generation.checked_sub(resume.initial_generation) != Some(resume.steps)
            || resume.steps > self.context.session.resources.maximum_steps()
            || resume.work > self.context.session.resources.maximum_work_units()
            || resume
                .steps
                .checked_mul(self.descriptor.resources.maximum_work_per_step)
                != Some(resume.work)
        {
            return Err(Error::IncompatibleCheckpoint);
        }
        // Verify every member before creating candidate state; never partially load self.
        let inference = store.read_blob(&descriptor.inference)?;
        let model_bytes = burn::tensor::Bytes::from_bytes_vec(store.read_blob(&resume.model)?);
        let optimizer_bytes =
            burn::tensor::Bytes::from_bytes_vec(store.read_blob(&resume.optimizer)?);
        let model_ids = crate::record_validation::validate_record(model_bytes.clone(), false)
            .map_err(|_| Error::CorruptCheckpoint)?;
        let optimizer_ids =
            crate::record_validation::validate_record(optimizer_bytes.clone(), true)
                .map_err(|_| Error::CorruptCheckpoint)?;
        if !optimizer_ids.is_subset(&model_ids) {
            return Err(Error::CorruptCheckpoint);
        }
        let record = ModuleRecord::from_bytes(model_bytes).map_err(|_| Error::CorruptCheckpoint)?;
        let model = self
            .model
            .as_ref()
            .ok_or(Error::Unloaded)?
            .clone()
            .fork(&self.device)
            .try_load_record(record)
            .map_err(|_| Error::CorruptCheckpoint)?;
        // Burn 0.22 loads optimizer tensors on the process-default device. Refuse
        // mismatches before invoking that API instead of silently choosing CUDA 0.
        if self.device != burn::tensor::Device::default() {
            return Err(Error::UnsupportedResumeDevice);
        }
        let mut model = model.train();
        for group in &self.descriptor.groups {
            if !group.trainable {
                let binding = self.definition.parameter_group(&model, &group.identity)?;
                model = model.freeze_group(binding);
            }
        }
        let optimizer_record = burn::optim::OptimizerRecord::from_bytes(optimizer_bytes)
            .map_err(|_| Error::CorruptCheckpoint)?;
        let recorded_tensors = optimizer_record.len();
        let candidate_optimizer = self.recipe.optimizer()?;
        let optimizer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            candidate_optimizer.load_record(optimizer_record)
        }))
        .map_err(|_| Error::CorruptCheckpoint)?;
        if optimizer.to_record().len() != recorded_tensors {
            return Err(Error::CorruptCheckpoint);
        }
        // The module record persists ParamIds; SafeTensors alone would silently lose
        // the association between parameters and AdamW's state on a fresh runtime.
        let mut check = SafetensorsStore::from_bytes(None).clear_metadata();
        model
            .save_into(&mut check)
            .map_err(|_| Error::CorruptCheckpoint)?;
        if check.get_bytes().map_err(|_| Error::CorruptCheckpoint)? != inference {
            return Err(Error::CorruptCheckpoint);
        }
        self.model = Some(model);
        self.optimizer = optimizer;
        self.state.model.generation = resume.generation;
        self.state.completed_steps = resume.steps;
        self.state.consumed_work_units = resume.work;
        self.inference_only = false;
        self.inference_generation = None;
        self.inference_checkpoint_identity = Some(*identity);
        Ok(())
    }
    fn read_descriptor(
        &self,
        store: &DirectoryCheckpointStore,
        id: &[u8; 32],
    ) -> Result<Descriptor, Error> {
        crate::checkpoint_manifest::read_descriptor(
            &self.descriptor,
            &self.context.artifact,
            store,
            id,
        )
    }
    fn context_identity(&self) -> Result<[u8; 32], Error> {
        let mut bytes = self
            .context
            .session
            .semantic_digest(
                &self.context.artifact,
                &self.context.dataset,
                &self.context.split,
            )?
            .to_vec();
        bytes.extend(serde_json::to_vec(&self.recipe).map_err(|_| Error::InvalidDescriptor)?);
        bytes.extend(self.descriptor.config_identity);
        bytes.extend(self.group_identity()?);
        Ok(digest(&bytes))
    }
    fn group_identity(&self) -> Result<[u8; 32], Error> {
        Ok(digest(
            &serde_json::to_vec(&self.descriptor.groups).map_err(|_| Error::InvalidDescriptor)?,
        ))
    }
    fn checkpoint_ref(&self, id: [u8; 32], bytes: u64, generation: u64) -> ModelCheckpoint {
        ModelCheckpoint {
            base_artifact_identity: self.context.artifact.content_identity(),
            architecture_profile: self.descriptor.architecture.clone(),
            state_schema_version: self.descriptor.checkpoint_schema,
            generation,
            content: BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest(id),
                content_profile: KindId::from(MODEL_CHECKPOINT_INFO_ID),
                access_class: ResourceClassId::from("training-store/read@1"),
                extent: ResourceExtent { bytes, items: None },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest(id),
                    expires_at: None,
                },
            },
        }
    }
}
