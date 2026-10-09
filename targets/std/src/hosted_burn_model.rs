//! Inference off-ramp into the existing finite hosted model-compute boundary.
use super::{
    ModelComputeAdapter, ModelComputeAdapterTerminal, ModelComputeExecution, ModelComputeInvocation,
};
use conduit_ai::{ModelComputeOffer, ModelComputeOperation, ModelComputeRefusal};
use conduit_burn_model::{
    BurnAdapter, BurnModelDefinition, Cancellation, Error, InferenceBurnAdapter,
};

/// Prepared immutable inference checkpoint. Training remains on the adapter's
/// existing TrainingSession boundary; this single-tensor interface offers inference.
pub struct HostedBurnModelComputeAdapter<D: BurnModelDefinition> {
    adapter: InferenceBurnAdapter<D>,
    offer: ModelComputeOffer,
    cancellation: Cancellation,
}
impl<D: BurnModelDefinition> HostedBurnModelComputeAdapter<D> {
    pub fn from_checkpoint(adapter: BurnAdapter<D>) -> Result<Self, Error> {
        Self::from_inference(adapter.into_inference()?)
    }
    pub fn from_inference(adapter: InferenceBurnAdapter<D>) -> Result<Self, Error> {
        if adapter.signature().inputs.get().len() != 1
            || adapter.signature().outputs.get().len() != 1
        {
            return Err(Error::InvalidDescriptor);
        }
        let mut offer = adapter.offer().clone();
        if !offer
            .supported_operations
            .contains(&ModelComputeOperation::Inference)
        {
            return Err(Error::InvalidDescriptor);
        }
        offer.supported_operations = vec![ModelComputeOperation::Inference];
        offer.checkpoint_writing = false;
        offer.validate()?;
        Ok(Self {
            adapter,
            offer,
            cancellation: Cancellation::default(),
        })
    }
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }
    pub fn checkpoint_identity(&self) -> [u8; 32] {
        self.adapter
            .inference_checkpoint_identity()
            .expect("immutable inference adapter")
    }
}
impl<D: BurnModelDefinition> ModelComputeAdapter for HostedBurnModelComputeAdapter<D> {
    fn offer(&self) -> &ModelComputeOffer {
        &self.offer
    }
    fn load(&mut self, identity: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        // Host preparation already verified and loaded the content-bound snapshot.
        // This boundary cannot reinterpret a different artifact or admit fewer bytes.
        if identity != self.adapter.loaded_artifact_identity() {
            return Err(ModelComputeRefusal::ProviderUnavailable);
        }
        if bytes != self.adapter.model_bytes() {
            return Err(ModelComputeRefusal::ResourceBoundExceeded);
        }
        self.adapter
            .ready_for_inference()
            .map_err(|_| ModelComputeRefusal::ProviderUnavailable)
    }
    fn execute(&mut self, invocation: ModelComputeInvocation) -> ModelComputeAdapterTerminal {
        if self.cancellation.is_cancelled() {
            return ModelComputeAdapterTerminal::Cancelled;
        }
        if invocation.request_identity == [0; 32]
            || invocation.artifact_identity != self.adapter.loaded_artifact_identity()
            || invocation.checkpoint_identity != Some(self.checkpoint_identity())
        {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::ProviderUnavailable);
        }
        if let Err(error) = self.offer.admits(&invocation.requirement) {
            return ModelComputeAdapterTerminal::Refused(error);
        }
        if let Err(error) = self
            .adapter
            .admit_inference_requirement(&invocation.requirement)
        {
            return ModelComputeAdapterTerminal::Refused(match error {
                Error::ModelCompute(error) => error,
                _ => ModelComputeRefusal::ResourceBoundExceeded,
            });
        }
        if invocation.input.dimensions.len() != invocation.requirement.rank as usize {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        }
        if invocation
            .input
            .axes
            .iter()
            .zip(invocation.input.dimensions.iter())
            .any(|(axis, dimension)| {
                axis.role == conduit_data::TensorAxisRole::Batch
                    && *dimension > u64::from(invocation.requirement.batch_items)
            })
        {
            return ModelComputeAdapterTerminal::Refused(
                ModelComputeRefusal::ResourceBoundExceeded,
            );
        }
        if invocation.input.byte_count().ok() != Some(invocation.requirement.input_bytes) {
            return ModelComputeAdapterTerminal::Refused(
                ModelComputeRefusal::ResourceBoundExceeded,
            );
        }
        let input_identity = invocation.input.content_digest;
        match self
            .adapter
            .infer_cancellable(&[invocation.input], &self.cancellation)
        {
            Ok(mut outputs) if outputs.len() == 1 => {
                let output = outputs.remove(0);
                if output
                    .byte_count()
                    .ok()
                    .is_none_or(|bytes| bytes > invocation.requirement.output_bytes)
                {
                    return ModelComputeAdapterTerminal::Refused(
                        ModelComputeRefusal::ResourceBoundExceeded,
                    );
                }
                ModelComputeAdapterTerminal::Produced(Box::new(ModelComputeExecution {
                    request_identity: invocation.request_identity,
                    artifact_identity: invocation.artifact_identity,
                    checkpoint_identity: invocation.checkpoint_identity,
                    input_identity,
                    output,
                    consumed_work_units: self.adapter.admitted_inference_work(),
                    runtime: self.adapter.runtime().clone(),
                }))
            }
            Err(Error::ModelCompute(error)) => ModelComputeAdapterTerminal::Refused(error),
            Err(Error::InvalidTensor) => {
                ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape)
            }
            Err(Error::Cancelled) => ModelComputeAdapterTerminal::Cancelled,
            Err(Error::Unloaded) => ModelComputeAdapterTerminal::ProviderLost,
            _ => ModelComputeAdapterTerminal::Failed,
        }
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal, ModelComputeRefusal> {
        self.adapter
            .ready_for_inference()
            .map_err(|_| ModelComputeRefusal::InvalidLifecycleTransition)?;
        self.cancellation.cancel();
        Ok(ModelComputeAdapterTerminal::Cancelled)
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        self.adapter
            .unload()
            .map_err(|_| ModelComputeRefusal::InvalidLifecycleTransition)
    }
}
