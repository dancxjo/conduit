//! Preparation of an immutable model for the ordinary named-tensor Fore.
use crate::hosted_model_compute::ModelComputeAdapter;
use conduit_ai::model_infer_catalog::{model_infer_contract, signature_configuration};
use conduit_ai::{AdmittedModelResource, ModelComputeOperation, ModelComputeRequirement};
use conduit_core::*;

mod imported;
pub use imported::ImportedModelPrepareRefusal;
mod resource_custody;
use resource_custody::ModelCustody;

pub const MODEL_INFER_OPERATION: &str = "conduit.host/model-infer@1";
pub const MODEL_INFER_IMPLEMENTATION: &str = "std/model-compute@1";
pub const MODEL_INFER_PROFILE: &str = "conduit.model/named-inference@1";
pub const MODEL_INFER_MAXIMUM_BYTES: u32 = MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;

/// This owner is prepared once. It never discovers models or selects a provider
/// while Play is advancing. The existing adapter retains its compute session.
pub struct PreparedModelInfer<A: ModelComputeAdapter> {
    model: ModelCustody,
    adapter: A,
    requirement: ModelComputeRequirement,
    maximum_input_bytes: u64,
    capability: CapabilityOffer,
    signature_profile: KindId,
    resource: ResourceOffer,
    next_request: u32,
    cancelled: bool,
    unloaded: bool,
    pending: Option<execution::PendingInference>,
    pending_input: Vec<u8>,
    runtime: conduit_ai::ModelComputeRuntimeIdentity,
    output_reader: conduit_data::PreparedNamedTensorBundleView,
    prepared_output: Option<conduit_data::PreparedNamedTensorBundleOutput>,
    input_reader: conduit_data::PreparedNamedTensorBundleView,
    signature_io: conduit_ai::PreparedModelTensorIo,
}

impl<A: ModelComputeAdapter> PreparedModelInfer<A> {
    pub fn prepare(
        model: AdmittedModelResource,
        adapter: A,
        requirement: ModelComputeRequirement,
        adapter_artifact: ArtifactId,
    ) -> Result<Self, String> {
        Self::prepare_owned(
            ModelCustody::Native(model),
            adapter,
            requirement,
            adapter_artifact,
        )
    }

    pub fn imported_model(&self) -> Option<&conduit_ai::imported_onnx::ImportedOnnxResource> {
        self.model.imported()
    }

    /// Actual prepared provider identity, borrowed from the existing adapter session.
    pub fn runtime_identity(&self) -> &conduit_ai::ModelComputeRuntimeIdentity {
        self.adapter.runtime_identity()
    }

    /// Retained exact checkpoint/Back receipt corresponding to selected Plan inspection.
    pub fn imported_admission(
        &self,
    ) -> Option<&conduit_ai::imported_back_admission::ImportedBackAdmissionReceipt> {
        self.model.imported_admission()
    }

    fn prepare_owned(
        model: ModelCustody,
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
        let signature_profile = property.profile().clone();
        let kind = model_infer_contract()?;
        let identity = model
            .selection_identity()
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
        let input_reader = conduit_data::PreparedNamedTensorBundleView::prepare()
            .map_err(|e| format!("model input preparation: {e}"))?;
        let signature_io = conduit_ai::PreparedModelTensorIo::prepare(model.signature())
            .map_err(|e| format!("model signature preparation: {e:?}"))?;
        let prepared_output = adapter
            .output_template()
            .map_err(|e| format!("model output preparation: {e:?}"))?
            .map(|template| {
                model
                    .signature()
                    .validate_named_outputs(&template)
                    .map_err(|e| format!("model output template differs from signature: {e:?}"))?;
                if template
                    .tensor_bytes()
                    .map_err(|e| format!("model output bound: {e:?}"))?
                    > requirement.output_bytes
                {
                    return Err("model output template exceeds admitted bytes".to_string());
                }
                let output = conduit_data::PreparedNamedTensorBundleOutput::prepare(&template)
                    .map_err(|e| format!("model canonical output preparation: {e:?}"))?;
                if output.maximum_encoded_len() > MODEL_INFER_MAXIMUM_BYTES as usize {
                    return Err("canonical model output exceeds the Host Call envelope".into());
                }
                Ok(output)
            })
            .transpose()?;
        adapter
            .load(model.artifact().content_identity(), requirement.model_bytes)
            .map_err(|e| format!("model load: {e:?}"))?;
        let runtime = adapter.runtime_identity().clone();
        let output_reader = conduit_data::PreparedNamedTensorBundleView::prepare()
            .map_err(|e| format!("model output reader preparation: {e}"))?;
        Ok(Self {
            model,
            adapter,
            maximum_input_bytes: requirement.input_bytes,
            requirement,
            capability,
            signature_profile,
            resource: ResourceOffer {
                pool_id: format!("model/infer/{identity}").into(),
                class_id: class.into(),
                capacity_units: 1,
                compute: None,
                content: None,
            },
            next_request: 1,
            cancelled: false,
            unloaded: false,
            pending: None,
            pending_input: Vec::with_capacity(MODEL_INFER_MAXIMUM_BYTES as usize),
            runtime,
            output_reader,
            prepared_output,
            input_reader,
            signature_io,
        })
    }
}

mod execution;

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
