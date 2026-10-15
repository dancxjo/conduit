//! Small concrete realizations of the generalized model-compute Host boundary.

use conduit_ai::{
    ModelComputeOffer, ModelComputeRefusal, ModelComputeRequirement, ModelComputeRuntimeIdentity,
    ModelComputeSession,
};
use conduit_data::{
    tensor_content_digest, NamedTensor, NamedTensorBundle, TensorBacking, TensorElement,
    TensorValue,
};
use conduit_plot::rust_binding::BoundedBytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelComputeInvocation {
    pub request_identity: [u8; 32],
    pub artifact_identity: [u8; 32],
    pub checkpoint_identity: Option<[u8; 32]>,
    pub requirement: ModelComputeRequirement,
    pub inputs: NamedTensorBundle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelComputeExecution {
    pub request_identity: [u8; 32],
    pub artifact_identity: [u8; 32],
    pub checkpoint_identity: Option<[u8; 32]>,
    pub inputs_identity: [u8; 32],
    pub outputs: NamedTensorBundle,
    pub consumed_work_units: u64,
    pub runtime: ModelComputeRuntimeIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelComputeAdapterTerminal {
    Produced(Box<ModelComputeExecution>),
    Refused(ModelComputeRefusal),
    Cancelled,
    ProviderLost,
    Failed,
}

/// One finite semantic invocation boundary. Framework-specific methods and
/// device handles remain behind implementations of this trait.
pub trait ModelComputeAdapter {
    fn offer(&self) -> &ModelComputeOffer;
    fn load(
        &mut self,
        artifact_identity: [u8; 32],
        model_bytes: u64,
    ) -> Result<(), ModelComputeRefusal>;
    fn execute(&mut self, invocation: ModelComputeInvocation) -> ModelComputeAdapterTerminal;
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal, ModelComputeRefusal>;
    fn unload(&mut self) -> Result<(), ModelComputeRefusal>;
}

pub struct ReferenceModelComputeAdapter {
    offer: ModelComputeOffer,
    session: ModelComputeSession,
}

pub struct LinearF32ModelAdapter {
    offer: ModelComputeOffer,
    session: ModelComputeSession,
    weights: [[f32; 2]; 2],
    bias: [f32; 2],
}

impl ReferenceModelComputeAdapter {
    pub fn new(
        offer: ModelComputeOffer,
        runtime: ModelComputeRuntimeIdentity,
    ) -> Result<Self, ModelComputeRefusal> {
        let session = ModelComputeSession::discovered(offer.clone(), runtime)?;
        Ok(Self { offer, session })
    }
}

impl LinearF32ModelAdapter {
    pub fn new(
        offer: ModelComputeOffer,
        runtime: ModelComputeRuntimeIdentity,
        weights: [[f32; 2]; 2],
        bias: [f32; 2],
    ) -> Result<Self, ModelComputeRefusal> {
        let session = ModelComputeSession::discovered(offer.clone(), runtime)?;
        Ok(Self {
            offer,
            session,
            weights,
            bias,
        })
    }
}

impl ModelComputeAdapter for ReferenceModelComputeAdapter {
    fn offer(&self) -> &ModelComputeOffer {
        &self.offer
    }
    fn load(&mut self, identity: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        load(&mut self.session, identity, bytes)
    }
    fn execute(&mut self, invocation: ModelComputeInvocation) -> ModelComputeAdapterTerminal {
        let input = match validate_invocation(&self.offer, &self.session, &invocation) {
            Ok(value) => value,
            Err(error) => return ModelComputeAdapterTerminal::Refused(error),
        };
        if let Err(error) = self.session.begin(&invocation.requirement, 0) {
            return ModelComputeAdapterTerminal::Refused(error);
        }
        let outputs = invocation.inputs.clone();
        if outputs
            .tensor_bytes()
            .ok()
            .is_none_or(|n| n > invocation.requirement.output_bytes)
        {
            let _ = self.session.finish();
            return ModelComputeAdapterTerminal::Refused(
                ModelComputeRefusal::ResourceBoundExceeded,
            );
        }
        let runtime = self.session.runtime().clone();
        let _ = self.session.finish();
        ModelComputeAdapterTerminal::Produced(Box::new(ModelComputeExecution {
            request_identity: invocation.request_identity,
            artifact_identity: invocation.artifact_identity,
            checkpoint_identity: None,
            inputs_identity: input,
            outputs,
            consumed_work_units: 1,
            runtime,
        }))
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal, ModelComputeRefusal> {
        self.session.cancel()?;
        Ok(ModelComputeAdapterTerminal::Cancelled)
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        unload(&mut self.session)
    }
}

impl ModelComputeAdapter for LinearF32ModelAdapter {
    fn offer(&self) -> &ModelComputeOffer {
        &self.offer
    }
    fn load(&mut self, identity: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        load(&mut self.session, identity, bytes)
    }
    fn execute(&mut self, invocation: ModelComputeInvocation) -> ModelComputeAdapterTerminal {
        let inputs_identity = match validate_invocation(&self.offer, &self.session, &invocation) {
            Ok(value) => value,
            Err(error) => return ModelComputeAdapterTerminal::Refused(error),
        };
        let inputs = invocation.inputs.get().as_slice();
        if inputs.len() != 1 || inputs[0].name != "input" {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        }
        let input = &inputs[0].value;
        if input.element != TensorElement::F32 || input.dimensions.as_slice() != [2] {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        }
        let TensorBacking::Inline(bytes) = &input.backing else {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedFormat);
        };
        let bytes = bytes.as_slice();
        let values = [
            f32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        ];
        if let Err(error) = self.session.begin(&invocation.requirement, 0) {
            return ModelComputeAdapterTerminal::Refused(error);
        }
        let result = [
            self.weights[0][0] * values[0] + self.weights[0][1] * values[1] + self.bias[0],
            self.weights[1][0] * values[0] + self.weights[1][1] * values[1] + self.bias[1],
        ];
        let output_bytes = result
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let output = TensorValue {
            element: TensorElement::F32,
            dimensions: input.dimensions.clone(),
            axes: input.axes.clone(),
            content_digest: tensor_content_digest(&output_bytes),
            backing: TensorBacking::Inline(
                BoundedBytes::new(&output_bytes).expect("two f32 values fit the tensor bound"),
            ),
        };
        let outputs = match NamedTensorBundle::from_values(vec![NamedTensor {
            name: "output".into(),
            value: output,
        }]) {
            Ok(value)
                if value
                    .tensor_bytes()
                    .ok()
                    .is_some_and(|n| n <= invocation.requirement.output_bytes) =>
            {
                value
            }
            _ => {
                let _ = self.session.finish();
                return ModelComputeAdapterTerminal::Refused(
                    ModelComputeRefusal::ResourceBoundExceeded,
                );
            }
        };
        let runtime = self.session.runtime().clone();
        let _ = self.session.finish();
        ModelComputeAdapterTerminal::Produced(Box::new(ModelComputeExecution {
            request_identity: invocation.request_identity,
            artifact_identity: invocation.artifact_identity,
            checkpoint_identity: None,
            inputs_identity,
            outputs,
            consumed_work_units: 8,
            runtime,
        }))
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal, ModelComputeRefusal> {
        self.session.cancel()?;
        Ok(ModelComputeAdapterTerminal::Cancelled)
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        unload(&mut self.session)
    }
}

fn validate_invocation(
    offer: &ModelComputeOffer,
    session: &ModelComputeSession,
    invocation: &ModelComputeInvocation,
) -> Result<[u8; 32], ModelComputeRefusal> {
    if invocation.request_identity == [0; 32] || invocation.artifact_identity == [0; 32] {
        return Err(ModelComputeRefusal::MissingIdentity);
    }
    if invocation.checkpoint_identity.is_some() {
        return Err(ModelComputeRefusal::ProviderUnavailable);
    }
    if session.loaded_model_identity() != Some(invocation.artifact_identity) {
        return Err(ModelComputeRefusal::ProviderUnavailable);
    }
    offer.admits(&invocation.requirement)?;
    invocation
        .inputs
        .validate()
        .map_err(|_| ModelComputeRefusal::UnsupportedShape)?;
    for input in invocation.inputs.get().iter() {
        if !invocation
            .requirement
            .elements
            .contains(&input.value.element)
        {
            return Err(ModelComputeRefusal::UnsupportedElement);
        }
        if input.value.dimensions.len() > usize::from(invocation.requirement.rank) {
            return Err(ModelComputeRefusal::UnsupportedShape);
        }
    }
    if invocation.inputs.tensor_bytes().ok() != Some(invocation.requirement.input_bytes) {
        return Err(ModelComputeRefusal::ResourceBoundExceeded);
    }
    invocation
        .inputs
        .semantic_digest()
        .map_err(|_| ModelComputeRefusal::UnsupportedShape)
}
fn load(
    session: &mut ModelComputeSession,
    identity: [u8; 32],
    bytes: u64,
) -> Result<(), ModelComputeRefusal> {
    session.begin_load(identity, bytes)?;
    session.begin_warming()?;
    session.ready()
}
fn unload(session: &mut ModelComputeSession) -> Result<(), ModelComputeRefusal> {
    session.begin_unload()?;
    session.shutdown()
}

#[cfg(feature = "burn-model")]
#[path = "hosted_burn_model.rs"]
mod burn_model;
#[cfg(feature = "burn-model")]
pub use burn_model::HostedBurnModelComputeAdapter;
