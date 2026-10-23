//! Custody of one exact asynchronous invocation and its existing success boundary.
use super::*;
use crate::hosted_model_compute::{ModelComputeAdapterTerminal, ModelComputeInvocation};
use crate::hosted_model_work::{HostedModelWorkAdapter, ModelWorkTerminal};

#[derive(Clone, Copy)]
pub(super) struct PendingInference {
    request: [u8; 32],
    inputs: [u8; 32],
}

impl<A: ModelComputeAdapter + Send> HostedModelWorkAdapter for PreparedModelInfer<A> {
    fn capability_offer(&self) -> &CapabilityOffer {
        &self.capability
    }
    fn resource_offer(&self) -> &ResourceOffer {
        &self.resource
    }
    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> ModelWorkTerminal {
        output.clear();
        if self.cancelled {
            return ModelWorkTerminal::Cancelled;
        }
        if self.unloaded {
            return ModelWorkTerminal::ProviderLost;
        }
        if self.pending.is_some() {
            return ModelWorkTerminal::Refused;
        }
        let [configuration] = placement.configuration.as_slice() else {
            return ModelWorkTerminal::Refused;
        };
        let ConfigurationValue::Structured(signature) = &configuration.value else {
            return ModelWorkTerminal::Refused;
        };
        if placement.capability_id != self.capability.capability_id
            || placement.artifact_id != self.capability.implementation.artifact_id
            || placement.implementation_id.as_str() != MODEL_INFER_IMPLEMENTATION
            || configuration.key != "signature"
            || placement.realization_properties != self.capability.realization_properties
            || self
                .capability
                .realization_properties
                .iter()
                .find(|p| p.profile() == &self.signature_profile)
                != Some(signature)
        {
            return ModelWorkTerminal::Refused;
        }
        if input.len() > self.pending_input.capacity() {
            return ModelWorkTerminal::MalformedInput;
        }
        self.pending_input.clear();
        self.pending_input.extend_from_slice(input);
        let Ok(inputs) = self.input_reader.read(&self.pending_input) else {
            return ModelWorkTerminal::MalformedInput;
        };
        if self.signature_io.validate_inputs(&inputs).is_err()
            || self.model.input_symbols(&inputs).is_err()
        {
            return ModelWorkTerminal::MalformedInput;
        }
        let bytes = inputs.tensor_bytes();
        if bytes > self.maximum_input_bytes {
            return ModelWorkTerminal::Refused;
        }
        let Some(next) = self.next_request.checked_add(1) else {
            return ModelWorkTerminal::Refused;
        };
        let mut request = self.model.descriptor_identity();
        request[..4].copy_from_slice(&self.next_request.to_le_bytes());
        self.next_request = next;
        self.requirement.input_bytes = bytes;
        let pending = PendingInference {
            request,
            inputs: inputs.semantic_digest(),
        };
        let terminal = self.adapter.execute(ModelComputeInvocation {
            request_identity: request,
            artifact_identity: self.model.artifact().content_identity(),
            checkpoint_identity: None,
            requirement: &self.requirement,
            inputs,
        });
        let result = Completion {
            model: &self.model,
            requirement: &self.requirement,
            runtime: &self.runtime,
            input: &self.pending_input,
            input_reader: &mut self.input_reader,
            output_reader: &mut self.output_reader,
            signature_io: &mut self.signature_io,
            prepared_output: &mut self.prepared_output,
        }
        .finish(pending, terminal, output);
        if result == ModelWorkTerminal::Pending {
            self.pending = Some(pending);
        } else {
            self.pending_input.clear();
        }
        result
    }
    fn poll(&mut self, output: &mut Vec<u8>) -> ModelWorkTerminal {
        output.clear();
        if self.cancelled {
            return ModelWorkTerminal::Cancelled;
        }
        if self.unloaded {
            return ModelWorkTerminal::ProviderLost;
        }
        let Some(pending) = self.pending.take() else {
            return ModelWorkTerminal::Failed;
        };
        let terminal = self.adapter.poll();
        let result = Completion {
            model: &self.model,
            requirement: &self.requirement,
            runtime: &self.runtime,
            input: &self.pending_input,
            input_reader: &mut self.input_reader,
            output_reader: &mut self.output_reader,
            signature_io: &mut self.signature_io,
            prepared_output: &mut self.prepared_output,
        }
        .finish(pending, terminal, output);
        if result == ModelWorkTerminal::Pending {
            self.pending = Some(pending);
        } else {
            self.pending_input.clear();
        }
        result
    }
    fn cancel(&mut self) {
        if self.cancelled || self.unloaded {
            return;
        }
        // Revoke publication before asking the backend to terminate its worker.
        self.cancelled = true;
        self.pending = None;
        self.pending_input.fill(0);
        self.pending_input.clear();
        let _ = self.adapter.cancel();
    }
    fn unload(&mut self) {
        self.retire();
    }
}

impl<A: ModelComputeAdapter> PreparedModelInfer<A> {
    fn retire(&mut self) {
        if self.unloaded {
            return;
        }
        self.unloaded = true;
        self.pending = None;
        self.pending_input.fill(0);
        self.pending_input.clear();
        let _ = self.adapter.unload();
    }
}
impl<A: ModelComputeAdapter> Drop for PreparedModelInfer<A> {
    fn drop(&mut self) {
        self.retire();
    }
}

struct Completion<'a> {
    model: &'a ModelCustody,
    requirement: &'a ModelComputeRequirement,
    runtime: &'a conduit_ai::ModelComputeRuntimeIdentity,
    input: &'a [u8],
    input_reader: &'a mut conduit_data::PreparedNamedTensorBundleView,
    output_reader: &'a mut conduit_data::PreparedNamedTensorBundleView,
    signature_io: &'a mut conduit_ai::PreparedModelTensorIo,
    prepared_output: &'a mut Option<conduit_data::PreparedNamedTensorBundleOutput>,
}
impl Completion<'_> {
    fn finish(
        &mut self,
        pending: PendingInference,
        terminal: ModelComputeAdapterTerminal<'_>,
        output: &mut Vec<u8>,
    ) -> ModelWorkTerminal {
        match terminal {
            ModelComputeAdapterTerminal::Pending => ModelWorkTerminal::Pending,
            ModelComputeAdapterTerminal::Produced(result) => {
                let Ok(inputs) = self.input_reader.read(self.input) else {
                    return ModelWorkTerminal::Failed;
                };
                let Ok(outputs) = self.output_reader.read(result.outputs.encoded()) else {
                    return ModelWorkTerminal::Failed;
                };
                let Ok(mut symbols) = self.model.input_symbols(&inputs) else {
                    return ModelWorkTerminal::Failed;
                };
                if result.request_identity != pending.request
                    || result.artifact_identity != self.model.artifact().content_identity()
                    || result.runtime != self.runtime
                    || self
                        .model
                        .imported_runtime()
                        .is_some_and(|runtime| result.runtime != runtime)
                    || result.inputs_identity != pending.inputs
                    || inputs.semantic_digest() != pending.inputs
                    || inputs.tensor_bytes() != self.requirement.input_bytes
                    || result.checkpoint_identity.is_some()
                    || self.signature_io.validate_inputs(&inputs).is_err()
                    || self.signature_io.validate_outputs(&outputs).is_err()
                    || symbols.outputs(&outputs).is_err()
                    || outputs.tensor_bytes() > self.requirement.output_bytes
                {
                    return ModelWorkTerminal::Failed;
                }
                let bytes = if let Some(prepared) = self.prepared_output.as_mut() {
                    prepared.reset();
                    for named in outputs.values() {
                        if prepared.write_view(named).is_err() {
                            return ModelWorkTerminal::Failed;
                        }
                    }
                    let Ok(bytes) = prepared.encoded() else {
                        return ModelWorkTerminal::Failed;
                    };
                    bytes
                } else {
                    result.outputs.encoded()
                };
                if bytes.len() > MODEL_INFER_MAXIMUM_BYTES as usize
                    || bytes.len() > output.capacity()
                {
                    return ModelWorkTerminal::Failed;
                }
                output.extend_from_slice(bytes);
                ModelWorkTerminal::Produced
            }
            ModelComputeAdapterTerminal::Refused(_) => ModelWorkTerminal::Refused,
            ModelComputeAdapterTerminal::Cancelled => ModelWorkTerminal::Cancelled,
            ModelComputeAdapterTerminal::ProviderLost => ModelWorkTerminal::ProviderLost,
            ModelComputeAdapterTerminal::TimedOut => ModelWorkTerminal::TimedOut,
            ModelComputeAdapterTerminal::Failed => ModelWorkTerminal::Failed,
        }
    }
}
