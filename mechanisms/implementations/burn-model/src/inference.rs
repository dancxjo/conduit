//! Immutable inference loading needs no dataset, training session, or optimizer.
use crate::model::validate_values;
use crate::{
    AuthoringDescriptor, BurnModelDefinition, Cancellation, DeviceRequest,
    DirectoryCheckpointStore, Error,
};
use burn::{
    module::Module,
    store::{ModuleSnapshot, SafetensorsStore},
    tensor::Device,
};
use conduit_ai::*;
use conduit_data::{TensorElement, TensorValue};

#[derive(Debug, Clone)]
pub struct InferenceContext {
    pub artifact: ModelArtifact,
    /// Actual host build/device evidence, validated before loading.
    pub runtime: ModelComputeRuntimeIdentity,
    pub determinism_profile: String,
}
pub struct InferenceBurnAdapter<D: BurnModelDefinition> {
    definition: D,
    descriptor: AuthoringDescriptor,
    device: Device,
    context: InferenceContext,
    model: Option<D::Model>,
    session: ModelComputeSession,
    offer: ModelComputeOffer,
    inference_checkpoint_identity: [u8; 32],
}
impl<D: BurnModelDefinition> InferenceBurnAdapter<D> {
    pub fn load(
        definition: D,
        request: DeviceRequest,
        context: InferenceContext,
        store: &DirectoryCheckpointStore,
        identity: &[u8; 32],
    ) -> Result<Self, Error> {
        let descriptor = definition.descriptor();
        descriptor.validate()?;
        if !descriptor.supported_profiles.contains(&request.evidence()) {
            return Err(Error::UnsupportedDevice);
        }
        context
            .artifact
            .validate(&descriptor.signature)
            .map_err(|_| Error::InvalidDescriptor)?;
        if context.artifact.architecture_profile != descriptor.architecture
            || context.artifact.state_schema_version != descriptor.checkpoint_schema
            || context.artifact.precision_profile != "number/ieee754-f32-le"
            || context.artifact.format_profile != "model/burn-safetensors@1"
            || context.runtime.runtime_name != "Burn"
            || context.runtime.runtime_version != "0.22.0"
            || context.runtime.device_evidence != request.evidence()
            || context.runtime.precision_profile != context.artifact.precision_profile
        {
            return Err(Error::InvalidDescriptor);
        }
        let checkpoint = crate::checkpoint_manifest::read_descriptor(
            &descriptor,
            &context.artifact,
            store,
            identity,
        )?;
        let bytes = store.read_blob(&checkpoint.inference)?;
        let device = request.prepare()?;
        let mut model = definition.initialize(&device)?.valid();
        if (model.num_params() as u64)
            .checked_mul(4)
            .ok_or(Error::ResourceBound)?
            > descriptor.resources.model_bytes
        {
            return Err(Error::ResourceBound);
        }
        let mut weights = SafetensorsStore::from_bytes(Some(bytes))
            .allow_partial(false)
            .validate(true);
        model
            .load_from(&mut weights)
            .map_err(|_| Error::CorruptCheckpoint)?;
        let model = model.valid();
        let record = model
            .clone()
            .into_record()
            .into_bytes()
            .map_err(|_| Error::CorruptCheckpoint)?;
        if record.len() as u64 > descriptor.resources.checkpoint_bytes {
            return Err(Error::ResourceBound);
        }
        crate::record_validation::validate_record(record, false)
            .map_err(|_| Error::CorruptCheckpoint)?;
        Self::from_loaded(definition, descriptor, device, model, context, *identity)
    }
    pub(crate) fn from_loaded(
        definition: D,
        descriptor: AuthoringDescriptor,
        device: Device,
        model: D::Model,
        context: InferenceContext,
        identity: [u8; 32],
    ) -> Result<Self, Error> {
        let mut limits = descriptor.limits;
        limits.compute.class = if context.runtime.device_evidence == DeviceRequest::Cpu.evidence() {
            PortableComputeClass::GeneralCpu
        } else {
            PortableComputeClass::Accelerator
        };
        if !descriptor
            .signature
            .operations
            .get()
            .as_slice()
            .contains(&ModelOperation::Infer)
        {
            return Err(Error::InvalidDescriptor);
        }
        let offer = ModelComputeOffer {
            identity: format!(
                "std/burn/{}/{}",
                descriptor.architecture, context.runtime.device_evidence
            ),
            supported_operations: vec![ModelComputeOperation::Inference],
            accepted_formats: vec![context.artifact.format_profile.clone()],
            supported_elements: vec![TensorElement::F32],
            solver_profiles: vec![],
            determinism_profiles: vec![context.determinism_profile.clone()],
            checkpoint_loading: true,
            checkpoint_writing: false,
            limits,
            cache_policy: ModelCachePolicy::bounded(limits.maximum_model_bytes, 1)
                .map_err(|_| Error::InvalidDescriptor)?,
        };
        offer.validate()?;
        let mut session = ModelComputeSession::discovered(offer.clone(), context.runtime.clone())?;
        session.begin_load(
            context.artifact.content_identity(),
            descriptor.resources.model_bytes,
        )?;
        session.begin_warming()?;
        session.ready()?;
        Ok(Self {
            definition,
            descriptor,
            device,
            model: Some(model.valid()),
            context,
            session,
            offer,
            inference_checkpoint_identity: identity,
        })
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
        Some(self.inference_checkpoint_identity)
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
        self.ready_for_inference_inner()
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
            determinism_profile: self.context.determinism_profile.clone(),
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
        self.ready_for_inference_inner()?;
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

    fn ready_for_inference_inner(&self) -> Result<(), Error> {
        if self.model.is_none() {
            return Err(Error::Unloaded);
        }
        Ok(())
    }
    pub fn unload(&mut self) -> Result<(), Error> {
        self.session.begin_unload()?;
        self.session.shutdown()?;
        self.model = None;
        Ok(())
    }
}
