//! Prepared Burn realization of provider-neutral model-session work.
//! Paths, devices and compiled model definitions stay in Host preparation.
use conduit_ai::{
    ModelWorkOperation, ModelWorkRefusal, ModelWorkReply, ModelWorkRequest, ModelWorkResult,
    TrainingRefusal,
};
use conduit_burn_model::{
    BurnAdapter, BurnModelDefinition, Cancellation, DeviceRequest, DirectoryCheckpointStore, Error,
    InferenceBurnAdapter, InferenceContext, ModelBatch,
};

pub struct HostedBurnWork<D: BurnModelDefinition, F: FnMut() -> D> {
    capability: conduit_core::CapabilityOffer,
    resource: conduit_core::ResourceOffer,
    training: BurnAdapter<D>,
    inference: Option<InferenceBurnAdapter<D>>,
    definition: F,
    device: DeviceRequest,
    context: InferenceContext,
    store: DirectoryCheckpointStore,
    cancellation: Cancellation,
}

impl<D: BurnModelDefinition, F: FnMut() -> D> HostedBurnWork<D, F> {
    pub fn prepare(
        training: BurnAdapter<D>,
        definition: F,
        device: DeviceRequest,
        context: InferenceContext,
        store: DirectoryCheckpointStore,
        process_identity: &str,
    ) -> Result<Self, Error> {
        // Establish every mutating receipt's wire bound before retaining this
        // provider. A committed update must never become an encoding failure.
        // TrainNext returns the same already-bounded TrainStepOutcome as Train;
        // no caller-sized tensor/cursor value enters its mutating reply.
        let reply_bound = conduit_ai::model_work_mutating_reply_bytes_bound()
            .map_err(|_| Error::InvalidDescriptor)?;
        if reply_bound > conduit_ai::MODEL_WORK_MAXIMUM_OUTPUT_BYTES as usize {
            return Err(Error::ResourceBound);
        }
        let retained = training.training_context();
        let realization = &retained.realization;
        if context.artifact != retained.artifact
            || context.runtime != *training.runtime()
            || context.determinism_profile != realization.deterministic_profile
            || device.evidence() != realization.device_profile
        {
            return Err(Error::InvalidDescriptor);
        }
        for identity in [
            &realization.implementation_identity,
            &realization.runtime_name,
            &realization.runtime_version,
            &realization.runtime_build_identity,
            &realization.device_profile,
            &realization.format_profile,
            &realization.precision_profile,
            &realization.deterministic_profile,
            &training.state().model.state_identity,
            &retained.artifact.architecture_profile,
        ] {
            if identity.is_empty() || identity.len() > conduit_ai::MAXIMUM_MODEL_IDENTITY_BYTES {
                return Err(Error::InvalidDescriptor);
            }
        }
        let capability =
            conduit_ai::model_work_offer(process_identity).map_err(|_| Error::InvalidDescriptor)?;
        let resource = conduit_core::ResourceOffer {
            pool_id: conduit_core::ResourcePoolId::from(format!(
                "model/session/{process_identity}"
            )),
            class_id: conduit_core::ResourceClassId::from(conduit_ai::MODEL_WORK_RESOURCE_CLASS),
            capacity_units: 1,
            compute: None,
            content: None,
        };
        Ok(Self {
            capability,
            resource,
            training,
            inference: None,
            definition,
            device,
            context,
            store,
            cancellation: Cancellation::default(),
        })
    }
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }
    pub fn execute(&mut self, request: ModelWorkRequest) -> Result<ModelWorkReply, Error> {
        request.validate().map_err(|_| Error::InvalidDescriptor)?;
        if self.cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if request.session_identity != self.training.state().session_identity {
            return Err(TrainingRefusal::InvalidSession.into());
        }
        if request.expected_generation != self.training.state().model.generation {
            return Err(TrainingRefusal::StaleState.into());
        }
        let result = match request.operation {
            ModelWorkOperation::TrainNext => {
                ModelWorkResult::Trained(self.training.train_next(&self.cancellation)?)
            }
            ModelWorkOperation::Train {
                request,
                inputs,
                targets,
            } => ModelWorkResult::Trained(self.training.train_step(
                &request,
                &ModelBatch { inputs, targets },
                &self.cancellation,
            )?),
            ModelWorkOperation::Evaluate {
                batch,
                inputs,
                targets,
            } => ModelWorkResult::Evaluated(
                self.training
                    .evaluate(&batch, &ModelBatch { inputs, targets })?,
            ),
            ModelWorkOperation::Checkpoint { metrics } => ModelWorkResult::Checkpointed(Box::new(
                self.training
                    .checkpoint(&self.store, metrics, &self.cancellation)?,
            )),
            ModelWorkOperation::Export => ModelWorkResult::Exported(
                self.training
                    .export_inference(&self.store, &self.cancellation)?,
            ),
            ModelWorkOperation::Resume {
                checkpoint_identity,
            } => {
                self.training.resume(&self.store, &checkpoint_identity)?;
                ModelWorkResult::Resumed(self.training.state().clone())
            }
            ModelWorkOperation::ReloadInference {
                checkpoint_identity,
            } => {
                let next = InferenceBurnAdapter::load(
                    (self.definition)(),
                    self.device,
                    self.context.clone(),
                    &self.store,
                    &checkpoint_identity,
                )?;
                // Failed loading retains the preceding inference owner.
                self.inference = Some(next);
                ModelWorkResult::ReloadedInference {
                    checkpoint_identity,
                }
            }
            ModelWorkOperation::Infer { inputs } => {
                let inference = self.inference.as_mut().ok_or(Error::Unloaded)?;
                let checkpoint_identity = inference.inference_checkpoint_identity();
                let outputs = inference.infer_cancellable(&inputs, &self.cancellation)?;
                ModelWorkResult::Inferred {
                    checkpoint_identity,
                    outputs,
                }
            }
        };
        Ok(ModelWorkReply {
            request_identity: request.request_identity,
            session_identity: request.session_identity,
            generation: self.training.state().model.generation,
            result,
        })
    }
}

impl<D, F> crate::hosted_model_work::HostedModelWorkAdapter for HostedBurnWork<D, F>
where
    D: BurnModelDefinition + Send,
    D::Model: Send,
    F: FnMut() -> D + Send,
{
    fn capability_offer(&self) -> &conduit_core::CapabilityOffer {
        &self.capability
    }
    fn resource_offer(&self) -> &conduit_core::ResourceOffer {
        &self.resource
    }
    fn cancel(&mut self) {
        self.cancellation.cancel();
    }
    fn execute(
        &mut self,
        placement: &conduit_core::PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> crate::hosted_model_work::ModelWorkTerminal {
        use crate::hosted_model_work::ModelWorkTerminal as Terminal;
        output.clear();
        if output.capacity() < conduit_ai::MODEL_WORK_MAXIMUM_OUTPUT_BYTES as usize {
            return Terminal::Refused;
        }
        if placement.capability_id != self.capability.capability_id
            || placement.implementation_id != self.capability.implementation.implementation_id
            || placement.artifact_id != self.capability.implementation.artifact_id
            || placement.execution_profile_id != self.capability.implementation.execution_profile_id
            || placement.resources.len() != 1
            || !placement.resources.iter().any(|binding| {
                binding.pool_id == self.resource.pool_id
                    && binding.class_id == self.resource.class_id
                    && binding.units == 1
                    && binding.protected.is_none()
                    && binding.compute.is_none()
                    && binding.content.is_none()
            })
        {
            return Terminal::Refused;
        }
        let Ok(request) = ModelWorkRequest::decode(input) else {
            return Terminal::MalformedInput;
        };
        let request_identity = request.request_identity;
        match self.execute(request) {
            Ok(reply) => match reply.encode() {
                Ok(bytes) if bytes.len() <= output.capacity() => {
                    output.extend_from_slice(&bytes);
                    Terminal::Produced
                }
                _ => Terminal::Failed,
            },
            Err(Error::Cancelled) => Terminal::Cancelled,
            Err(Error::Unloaded) => Terminal::ProviderLost,
            Err(error) => {
                let reason = match error {
                    Error::Training(reason) => ModelWorkRefusal::Training(reason),
                    Error::ModelCompute(reason) => ModelWorkRefusal::Compute(reason),
                    Error::ResourceBound => ModelWorkRefusal::ResourceBound,
                    Error::InvalidDescriptor => ModelWorkRefusal::InvalidDescriptor,
                    Error::InvalidTensor => ModelWorkRefusal::InvalidTensor,
                    Error::IncompatibleCheckpoint => ModelWorkRefusal::IncompatibleCheckpoint,
                    Error::CorruptCheckpoint => ModelWorkRefusal::CorruptCheckpoint,
                    Error::UnsupportedDevice => ModelWorkRefusal::UnsupportedDevice,
                    Error::UnsupportedResumeDevice => ModelWorkRefusal::UnsupportedResumeDevice,
                    Error::CheckpointIo(_) => ModelWorkRefusal::CheckpointIo,
                    Error::DurabilityUncertain(checkpoint_identity) => {
                        ModelWorkRefusal::DurabilityUncertain {
                            checkpoint_identity,
                        }
                    }
                    Error::NumericFailure => ModelWorkRefusal::NumericFailure,
                    Error::Cancelled | Error::Unloaded => {
                        unreachable!("terminal cases handled above")
                    }
                };
                let reply = ModelWorkReply {
                    request_identity,
                    session_identity: self.training.state().session_identity,
                    generation: self.training.state().model.generation,
                    result: ModelWorkResult::Refused(reason),
                };
                match reply.encode() {
                    Ok(bytes) if bytes.len() <= output.capacity() => {
                        output.extend_from_slice(&bytes);
                        Terminal::Produced
                    }
                    _ => Terminal::Failed,
                }
            }
        }
    }
}
