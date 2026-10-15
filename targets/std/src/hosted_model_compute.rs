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
    /// Provider reports that its admitted execution deadline expired.
    TimedOut,
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
    inputs: Vec<(String, usize)>,
    outputs: Vec<(String, usize)>,
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
            inputs: vec![("input".into(), 2)],
            outputs: vec![("output".into(), 2)],
        })
    }
}

impl LinearF32ModelAdapter {
    /// Port order belongs to the admitted semantic signature; bundle order does
    /// not change which feature enters each matrix column or leaves each row.
    pub fn with_signature(
        mut self,
        signature: &conduit_ai::ModelSignature,
    ) -> Result<Self, ModelComputeRefusal> {
        signature
            .validate()
            .map_err(|_| ModelComputeRefusal::UnsupportedShape)?;
        self.inputs = linear_ports(signature.inputs.get().as_slice())?;
        self.outputs = linear_ports(signature.outputs.get().as_slice())?;
        Ok(self)
    }
}
fn linear_ports(
    ports: &[conduit_ai::ModelPortConstraint],
) -> Result<Vec<(String, usize)>, ModelComputeRefusal> {
    let mut result = Vec::with_capacity(2);
    let mut total = 0_usize;
    for port in ports {
        let conduit_ai::ModelValueConstraint::Tensor(value) = &port.value else {
            return Err(ModelComputeRefusal::UnsupportedShape);
        };
        let tensor = value.constraint();
        let [axis] = tensor.axes.get().as_slice() else {
            return Err(ModelComputeRefusal::UnsupportedShape);
        };
        let conduit_ai::ModelDimensionConstraint::Fixed(width) = &axis.dimension else {
            return Err(ModelComputeRefusal::UnsupportedShape);
        };
        let width =
            usize::try_from(*width.value()).map_err(|_| ModelComputeRefusal::UnsupportedShape)?;
        if !(1..=2).contains(&width)
            || port.presence != conduit_ai::ModelPortPresence::Required
            || tensor.elements.get().as_slice() != [TensorElement::F32]
            || axis.role != conduit_data::TensorAxisRole::Feature
            || tensor.maximum_bytes != (width * 4) as u64
        {
            return Err(ModelComputeRefusal::UnsupportedShape);
        }
        total += width;
        if total > 2 {
            return Err(ModelComputeRefusal::UnsupportedShape);
        }
        result.push((port.identity.get().clone(), width));
    }
    if total != 2 {
        return Err(ModelComputeRefusal::UnsupportedShape);
    }
    Ok(result)
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
        if invocation.inputs.get().len() != self.inputs.len() {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        }
        let mut values = [0_f32; 2];
        let mut offset = 0;
        for (name, width) in &self.inputs {
            let Some(named) = invocation.inputs.get().iter().find(|v| &v.name == name) else {
                return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
            };
            let input = &named.value;
            if input.element != TensorElement::F32
                || input.dimensions.as_slice() != [*width as u64]
                || input
                    .axes
                    .iter()
                    .any(|axis| axis.role != conduit_data::TensorAxisRole::Feature)
            {
                return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
            }
            let TensorBacking::Inline(bytes) = &input.backing else {
                return ModelComputeAdapterTerminal::Refused(
                    ModelComputeRefusal::UnsupportedFormat,
                );
            };
            for bytes in bytes.as_slice().as_chunks::<4>().0 {
                values[offset] = f32::from_le_bytes(*bytes);
                offset += 1;
            }
        }
        if let Err(error) = self.session.begin(&invocation.requirement, 0) {
            return ModelComputeAdapterTerminal::Refused(error);
        }
        let result = [
            self.weights[0][0] * values[0] + self.weights[0][1] * values[1] + self.bias[0],
            self.weights[1][0] * values[0] + self.weights[1][1] * values[1] + self.bias[1],
        ];
        let mut named_outputs = Vec::with_capacity(self.outputs.len());
        let mut offset = 0;
        for (name, width) in &self.outputs {
            let output_bytes = result[offset..offset + width]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>();
            offset += width;
            let output = TensorValue {
                element: TensorElement::F32,
                dimensions: conduit_plot::rust_binding::BoundedSequence::try_from_iter([
                    *width as u64
                ])
                .expect("one dimension"),
                axes: conduit_plot::rust_binding::BoundedSequence::try_from_iter([
                    conduit_data::TensorAxis {
                        role: conduit_data::TensorAxisRole::Feature,
                        identity: None,
                        unit: None,
                    },
                ])
                .expect("one axis"),
                content_digest: tensor_content_digest(&output_bytes),
                backing: TensorBacking::Inline(
                    BoundedBytes::new(&output_bytes).expect("two f32 values fit"),
                ),
            };
            named_outputs.push(NamedTensor {
                name: name.clone(),
                value: output,
            });
        }
        let outputs = match NamedTensorBundle::from_values(named_outputs) {
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
