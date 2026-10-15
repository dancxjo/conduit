//! Preparation of an immutable model for the ordinary named-tensor Fore.
use crate::hosted_model_compute::{
    ModelComputeAdapter, ModelComputeAdapterTerminal, ModelComputeInvocation,
};
use crate::hosted_model_work::{HostedModelWorkAdapter, ModelWorkTerminal};
use conduit_ai::model_infer_catalog::{model_infer_contract, signature_configuration};
use conduit_ai::{AdmittedModelResource, ModelComputeOperation, ModelComputeRequirement};
use conduit_core::*;
use conduit_data::NamedTensorBundle;
use conduit_plot::rust_binding::NativeRustBinding;

pub const MODEL_INFER_OPERATION: &str = "conduit.host/model-infer@1";
pub const MODEL_INFER_IMPLEMENTATION: &str = "std/model-compute@1";
pub const MODEL_INFER_PROFILE: &str = "conduit.model/named-inference@1";
pub const MODEL_INFER_MAXIMUM_BYTES: u32 = MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;

/// This owner is prepared once. It never discovers models or selects a provider
/// while Play is advancing. The existing adapter retains its compute session.
pub struct PreparedModelInfer<A: ModelComputeAdapter> {
    model: AdmittedModelResource,
    adapter: A,
    requirement: ModelComputeRequirement,
    capability: CapabilityOffer,
    resource: ResourceOffer,
    next_request: u32,
}

impl<A: ModelComputeAdapter> PreparedModelInfer<A> {
    pub fn prepare(
        model: AdmittedModelResource,
        mut adapter: A,
        requirement: ModelComputeRequirement,
        adapter_artifact: ArtifactId,
    ) -> Result<Self, String> {
        if requirement.operation != ModelComputeOperation::Inference
            || requirement.model_format != model.artifact().format_profile
            || requirement.model_bytes != model.bytes().len() as u64
            || requirement.requires_checkpoint_load
            || requirement.requires_checkpoint_write
        {
            return Err("model inference preparation does not match admitted artifact".into());
        }
        for (ports, maximum) in [
            (
                model.signature().inputs.get().as_slice(),
                requirement.input_bytes,
            ),
            (
                model.signature().outputs.get().as_slice(),
                requirement.output_bytes,
            ),
        ] {
            let mut bytes = 0_u64;
            for port in ports {
                let constraint = match &port.value {
                    conduit_ai::ModelValueConstraint::Tensor(v) => v.constraint(),
                    _ => return Err("named tensor inference requires tensor ports".into()),
                };
                bytes = bytes
                    .checked_add(constraint.maximum_bytes)
                    .ok_or("model tensor bound overflow")?;
                if constraint.axes.get().iter().any(|axis| {
                    if axis.role != conduit_data::TensorAxisRole::Batch {
                        return false;
                    }
                    let maximum = match &axis.dimension {
                        conduit_ai::ModelDimensionConstraint::Fixed(v) => *v.value(),
                        conduit_ai::ModelDimensionConstraint::Bounded(v) => *v.maximum(),
                    };
                    maximum > u64::from(requirement.batch_items)
                }) {
                    return Err("model batch exceeds compute admission".into());
                }
                if constraint.axes.get().len() > usize::from(requirement.rank)
                    || constraint
                        .elements
                        .get()
                        .iter()
                        .any(|e| !requirement.elements.contains(e))
                {
                    return Err("model signature exceeds compute dtype/rank admission".into());
                }
            }
            if bytes > maximum {
                return Err("model signature exceeds admitted tensor bytes".into());
            }
        }
        adapter
            .offer()
            .admits(&requirement)
            .map_err(|e| format!("model compute admission: {e:?}"))?;
        let ConfigurationValue::Structured(property) = signature_configuration(model.signature())?
        else {
            return Err("model signature is not structured".into());
        };
        let kind = model_infer_contract()?;
        let identity = model
            .descriptor_identity()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let class = format!("model/infer/{identity}");
        let mut capability = BackOfferBuilder::new(
            kind.clone(),
            Back {
                capability_id: format!("model/infer/{identity}").into(),
                execution_profile_id: MODEL_INFER_PROFILE.into(),
                implementation_id: MODEL_INFER_IMPLEMENTATION.into(),
                artifact_id: adapter_artifact,
                host_calls: vec![HostCallRequirement {
                    contract_id: MODEL_INFER_OPERATION.into(),
                    target_kind: Some(kind.kind_id),
                    maximum_in_flight: 1,
                    maximum_input_bytes: MODEL_INFER_MAXIMUM_BYTES,
                    maximum_output_bytes: MODEL_INFER_MAXIMUM_BYTES,
                }],
                resource_requirements: vec![resource_requirement(&class, 1)],
                authority_requirements: vec![],
            },
        )
        .build();
        capability.realization_properties = vec![property];
        adapter
            .load(model.artifact().content_identity(), requirement.model_bytes)
            .map_err(|e| format!("model load: {e:?}"))?;
        Ok(Self {
            model,
            adapter,
            requirement,
            capability,
            resource: ResourceOffer {
                pool_id: format!("model/infer/{identity}").into(),
                class_id: class.into(),
                capacity_units: 1,
                compute: None,
                content: None,
            },
            next_request: 1,
        })
    }
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
        if placement.capability_id != self.capability.capability_id
            || placement.artifact_id != self.capability.implementation.artifact_id
            || placement.implementation_id.as_str() != MODEL_INFER_IMPLEMENTATION
            || conduit_ai::model_infer_catalog::configured_model_signature(&placement.configuration)
                .ok()
                .as_ref()
                != Some(self.model.signature())
        {
            return ModelWorkTerminal::Refused;
        }
        if input.len() > MODEL_INFER_MAXIMUM_BYTES as usize {
            return ModelWorkTerminal::MalformedInput;
        }
        let Ok(inputs) = NamedTensorBundle::decode(input) else {
            return ModelWorkTerminal::MalformedInput;
        };
        if self
            .model
            .signature()
            .validate_named_inputs(&inputs)
            .is_err()
        {
            return ModelWorkTerminal::MalformedInput;
        }
        let Ok(bytes) = inputs.tensor_bytes() else {
            return ModelWorkTerminal::MalformedInput;
        };
        if bytes > self.requirement.input_bytes {
            return ModelWorkTerminal::Refused;
        }
        let Some(next) = self.next_request.checked_add(1) else {
            return ModelWorkTerminal::Refused;
        };
        let mut identity = self.model.descriptor_identity();
        identity[..4].copy_from_slice(&self.next_request.to_le_bytes());
        self.next_request = next;
        let mut requirement = self.requirement.clone();
        requirement.input_bytes = bytes;
        let Ok(inputs_identity) = inputs.semantic_digest() else {
            return ModelWorkTerminal::MalformedInput;
        };
        let terminal = self.adapter.execute(ModelComputeInvocation {
            request_identity: identity,
            artifact_identity: self.model.artifact().content_identity(),
            checkpoint_identity: None,
            requirement,
            inputs,
        });
        match terminal {
            ModelComputeAdapterTerminal::Produced(result) => {
                if result.request_identity != identity
                    || result.artifact_identity != self.model.artifact().content_identity()
                    || result.inputs_identity != inputs_identity
                    || result.checkpoint_identity.is_some()
                    || self
                        .model
                        .signature()
                        .validate_named_outputs(&result.outputs)
                        .is_err()
                    || result
                        .outputs
                        .tensor_bytes()
                        .ok()
                        .is_none_or(|n| n > self.requirement.output_bytes)
                {
                    return ModelWorkTerminal::Failed;
                }
                let Ok(bytes) = result.outputs.encode() else {
                    return ModelWorkTerminal::Failed;
                };
                if bytes.len() > MODEL_INFER_MAXIMUM_BYTES as usize
                    || bytes.len() > output.capacity()
                {
                    return ModelWorkTerminal::Failed;
                }
                output.extend_from_slice(&bytes);
                ModelWorkTerminal::Produced
            }
            ModelComputeAdapterTerminal::Refused(_) => ModelWorkTerminal::Refused,
            ModelComputeAdapterTerminal::Cancelled => ModelWorkTerminal::Cancelled,
            ModelComputeAdapterTerminal::ProviderLost => ModelWorkTerminal::ProviderLost,
            ModelComputeAdapterTerminal::Failed => ModelWorkTerminal::Failed,
        }
    }
    fn cancel(&mut self) {
        let _ = self.adapter.cancel();
    }
}

/// The existing reference-linear profile stores six little-endian binary32
/// coefficients: two matrix rows followed by two biases. No weights supplied
/// separately by the caller can replace the admitted immutable content.
pub fn prepare_linear_model(
    model: AdmittedModelResource,
    offer: conduit_ai::ModelComputeOffer,
    runtime: conduit_ai::ModelComputeRuntimeIdentity,
    requirement: ModelComputeRequirement,
) -> Result<PreparedModelInfer<crate::hosted_model_compute::LinearF32ModelAdapter>, String> {
    if model.artifact().format_profile != "model/reference-linear"
        || model.artifact().state_schema_version != 1
        || model.bytes().len() != 24
    {
        return Err("unsupported reference-linear model artifact".into());
    }
    let mut coefficients = [0_f32; 6];
    for (value, bytes) in coefficients
        .iter_mut()
        .zip(model.bytes().as_chunks::<4>().0)
    {
        *value = f32::from_le_bytes(*bytes);
        if !value.is_finite() {
            return Err("non-finite reference-linear coefficient".into());
        }
    }
    let artifact = ArtifactId::from(runtime.adapter_artifact_identity.clone());
    let adapter = crate::hosted_model_compute::LinearF32ModelAdapter::new(
        offer,
        runtime,
        [
            [coefficients[0], coefficients[1]],
            [coefficients[2], coefficients[3]],
        ],
        [coefficients[4], coefficients[5]],
    )
    .map_err(|e| format!("reference-linear preparation: {e:?}"))?
    .with_signature(model.signature())
    .map_err(|e| format!("reference-linear signature: {e:?}"))?;
    PreparedModelInfer::prepare(model, adapter, requirement, artifact)
}
