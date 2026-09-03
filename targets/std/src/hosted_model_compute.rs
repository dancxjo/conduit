//! Small concrete realizations of the generalized model-compute Host boundary.

use conduit_ai::{
    ModelComputeOffer, ModelComputeRefusal, ModelComputeRequirement, ModelComputeRuntimeIdentity,
    ModelComputeSession,
};
use conduit_data::{tensor_content_digest, TensorBacking, TensorElement, TensorValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelComputeInvocation {
    pub request_identity: [u8; 32],
    pub artifact_identity: [u8; 32],
    pub requirement: ModelComputeRequirement,
    pub input: TensorValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelComputeExecution {
    pub request_identity: [u8; 32],
    pub artifact_identity: [u8; 32],
    pub input_identity: [u8; 32],
    pub output: TensorValue,
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
        let output = invocation.input.clone();
        let runtime = self.session.runtime().clone();
        let _ = self.session.finish();
        ModelComputeAdapterTerminal::Produced(Box::new(ModelComputeExecution {
            request_identity: invocation.request_identity,
            artifact_identity: invocation.artifact_identity,
            input_identity: input,
            output,
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
        let input_identity = match validate_invocation(&self.offer, &self.session, &invocation) {
            Ok(value) => value,
            Err(error) => return ModelComputeAdapterTerminal::Refused(error),
        };
        if invocation.input.element != TensorElement::F32 || invocation.input.dimensions != [2] {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        }
        let TensorBacking::Inline(bytes) = &invocation.input.backing else {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedFormat);
        };
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
            dimensions: vec![2],
            axes: invocation.input.axes.clone(),
            content_digest: tensor_content_digest(&output_bytes),
            backing: TensorBacking::Inline(output_bytes),
        };
        let runtime = self.session.runtime().clone();
        let _ = self.session.finish();
        ModelComputeAdapterTerminal::Produced(Box::new(ModelComputeExecution {
            request_identity: invocation.request_identity,
            artifact_identity: invocation.artifact_identity,
            input_identity,
            output,
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
    if session.loaded_model_identity() != Some(invocation.artifact_identity) {
        return Err(ModelComputeRefusal::ProviderUnavailable);
    }
    offer.admits(&invocation.requirement)?;
    invocation
        .input
        .validate()
        .map_err(|_| ModelComputeRefusal::UnsupportedShape)?;
    if invocation
        .input
        .byte_count()
        .map_err(|_| ModelComputeRefusal::UnsupportedShape)?
        != invocation.requirement.input_bytes
    {
        return Err(ModelComputeRefusal::ResourceBoundExceeded);
    }
    Ok(invocation.input.content_digest)
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
