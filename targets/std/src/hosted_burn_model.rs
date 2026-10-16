//! Inference off-ramp into the existing finite hosted model-compute boundary.
use super::{
    ModelComputeAdapter, ModelComputeAdapterTerminal, ModelComputeExecution, ModelComputeInvocation,
};
use conduit_ai::{ModelComputeOffer, ModelComputeOperation, ModelComputeRefusal};
use conduit_burn_model::{
    BurnAdapter, BurnModelDefinition, Cancellation, Error, InferenceBurnAdapter,
};

/// Prepared immutable inference checkpoint. Training remains on the adapter's
/// existing TrainingSession boundary; inference uses canonical named tensors.
pub struct HostedBurnModelComputeAdapter<D: BurnModelDefinition> {
    adapter: InferenceBurnAdapter<D>,
    offer: ModelComputeOffer,
    cancellation: Cancellation,
    output: Vec<u8>,
    output_reader: conduit_data::PreparedNamedTensorBundleView,
}
impl<D: BurnModelDefinition> HostedBurnModelComputeAdapter<D> {
    pub fn from_checkpoint(adapter: BurnAdapter<D>) -> Result<Self, Error> {
        Self::from_inference(adapter.into_inference()?)
    }
    pub fn from_inference(adapter: InferenceBurnAdapter<D>) -> Result<Self, Error> {
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
            output: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            output_reader: conduit_data::PreparedNamedTensorBundleView::prepare()
                .map_err(|_| Error::InvalidDescriptor)?,
        })
    }
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }
    pub fn signature(&self) -> &conduit_ai::ModelSignature {
        self.adapter.signature()
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
    fn execute(
        &mut self,
        invocation: ModelComputeInvocation<'_>,
    ) -> ModelComputeAdapterTerminal<'_> {
        if self.cancellation.is_cancelled() {
            return ModelComputeAdapterTerminal::Cancelled;
        }
        if invocation.request_identity == [0; 32]
            || invocation.artifact_identity != self.adapter.loaded_artifact_identity()
            || invocation.checkpoint_identity != Some(self.checkpoint_identity())
        {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::ProviderUnavailable);
        }
        if let Err(error) = self.offer.admits(invocation.requirement) {
            return ModelComputeAdapterTerminal::Refused(error);
        }
        if let Err(error) = self
            .adapter
            .admit_inference_requirement(invocation.requirement)
        {
            return ModelComputeAdapterTerminal::Refused(match error {
                Error::ModelCompute(error) => error,
                _ => ModelComputeRefusal::ResourceBoundExceeded,
            });
        }
        let Ok(inputs_owned) = <conduit_data::NamedTensorBundle as conduit_plot::rust_binding::NativeRustBinding>::decode(invocation.inputs.encoded()) else {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        };
        if self
            .adapter
            .signature()
            .validate_named_inputs(&inputs_owned)
            .is_err()
        {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        }
        if inputs_owned.get().iter().any(|input| {
            !invocation
                .requirement
                .elements
                .contains(&input.value.element)
                || input.value.dimensions.len() > usize::from(invocation.requirement.rank)
                || input
                    .value
                    .axes
                    .iter()
                    .zip(input.value.dimensions.iter())
                    .any(|(axis, n)| {
                        axis.role == conduit_data::TensorAxisRole::Batch
                            && *n > u64::from(invocation.requirement.batch_items)
                    })
        }) {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        }
        if invocation.inputs.tensor_bytes() != invocation.requirement.input_bytes {
            return ModelComputeAdapterTerminal::Refused(
                ModelComputeRefusal::ResourceBoundExceeded,
            );
        }
        let inputs_identity = invocation.inputs.semantic_digest();
        let inputs = self
            .adapter
            .signature()
            .inputs
            .get()
            .iter()
            .map(|port| {
                inputs_owned
                    .get()
                    .iter()
                    .find(|value| &value.name == port.identity.get())
                    .map(|value| value.value.clone())
            })
            .collect::<Option<Vec<_>>>();
        let Some(inputs) = inputs else {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        };
        match self.adapter.infer_cancellable(&inputs, &self.cancellation) {
            Ok(values) => {
                let ports = self.adapter.signature().outputs.get().as_slice();
                if values.len() != ports.len() {
                    return ModelComputeAdapterTerminal::Refused(
                        ModelComputeRefusal::UnsupportedShape,
                    );
                }
                let outputs = conduit_data::NamedTensorBundle::from_values(
                    ports
                        .iter()
                        .zip(values)
                        .map(|(port, value)| conduit_data::NamedTensor {
                            name: port.identity.get().clone(),
                            value,
                        })
                        .collect(),
                );
                let Ok(outputs) = outputs else {
                    return ModelComputeAdapterTerminal::Refused(
                        ModelComputeRefusal::UnsupportedShape,
                    );
                };
                if self
                    .adapter
                    .signature()
                    .validate_named_outputs(&outputs)
                    .is_err()
                    || outputs.get().iter().any(|output| {
                        !invocation
                            .requirement
                            .elements
                            .contains(&output.value.element)
                            || output.value.dimensions.len()
                                > usize::from(invocation.requirement.rank)
                    })
                    || outputs
                        .tensor_bytes()
                        .ok()
                        .is_none_or(|n| n > invocation.requirement.output_bytes)
                {
                    return ModelComputeAdapterTerminal::Refused(
                        ModelComputeRefusal::ResourceBoundExceeded,
                    );
                }
                let Ok(encoded) = <conduit_data::NamedTensorBundle as conduit_plot::rust_binding::NativeRustBinding>::encode(outputs) else {
                    return ModelComputeAdapterTerminal::Failed;
                };
                if encoded.len() > self.output.capacity() {
                    return ModelComputeAdapterTerminal::Refused(
                        ModelComputeRefusal::ResourceBoundExceeded,
                    );
                }
                self.output.clear();
                self.output.extend_from_slice(&encoded);
                let Ok(outputs) = self.output_reader.read(&self.output) else {
                    return ModelComputeAdapterTerminal::Failed;
                };
                ModelComputeAdapterTerminal::Produced(ModelComputeExecution {
                    request_identity: invocation.request_identity,
                    artifact_identity: invocation.artifact_identity,
                    checkpoint_identity: invocation.checkpoint_identity,
                    inputs_identity,
                    outputs: outputs.encoded_ref(),
                    consumed_work_units: self.adapter.admitted_inference_work(),
                    runtime: self.adapter.runtime(),
                })
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
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
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
