//! Small concrete realizations of the generalized model-compute Host boundary.

use conduit_ai::{
    ModelComputeOffer, ModelComputeRefusal, ModelComputeRequirement, ModelComputeRuntimeIdentity,
    ModelComputeSession,
};
use conduit_data::{
    tensor_content_digest, NamedTensor, NamedTensorBundle, NamedTensorBundleRef,
    NamedTensorBundleView, PreparedNamedTensorBundleOutput, PreparedNamedTensorBundleView,
    TensorBacking, TensorBackingView, TensorElement, TensorValue,
};
use conduit_plot::rust_binding::BoundedBytes;

#[derive(Clone)]
pub struct ModelComputeInvocation<'a> {
    pub request_identity: [u8; 32],
    pub artifact_identity: [u8; 32],
    pub checkpoint_identity: Option<[u8; 32]>,
    pub requirement: &'a ModelComputeRequirement,
    pub inputs: NamedTensorBundleView<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelComputeExecution<'a> {
    pub request_identity: [u8; 32],
    pub artifact_identity: [u8; 32],
    pub checkpoint_identity: Option<[u8; 32]>,
    pub inputs_identity: [u8; 32],
    pub outputs: NamedTensorBundleRef<'a>,
    pub consumed_work_units: u64,
    pub runtime: &'a ModelComputeRuntimeIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelComputeAdapterTerminal<'a> {
    Produced(ModelComputeExecution<'a>),
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
    /// Identity of the actual prepared provider session, before model loading.
    fn runtime_identity(&self) -> &ModelComputeRuntimeIdentity;
    fn load(
        &mut self,
        artifact_identity: [u8; 32],
        model_bytes: u64,
    ) -> Result<(), ModelComputeRefusal>;
    /// Optional exact output geometry owned by this prepared provider profile.
    /// It is checked against the admitted model before execution. A provider
    /// declaring this template must preserve its complete tensor metadata.
    fn output_template(&self) -> Result<Option<NamedTensorBundle>, ModelComputeRefusal> {
        Ok(None)
    }
    fn execute(
        &mut self,
        invocation: ModelComputeInvocation<'_>,
    ) -> ModelComputeAdapterTerminal<'_>;
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal>;
    fn unload(&mut self) -> Result<(), ModelComputeRefusal>;
}

pub struct ReferenceModelComputeAdapter {
    offer: ModelComputeOffer,
    session: ModelComputeSession,
    output: Vec<u8>,
    reader: PreparedNamedTensorBundleView,
}

pub struct LinearF32ModelAdapter {
    offer: ModelComputeOffer,
    session: ModelComputeSession,
    weights: [[f32; 2]; 2],
    bias: [f32; 2],
    inputs: Vec<(String, usize)>,
    outputs: Vec<(String, usize)>,
    output: PreparedNamedTensorBundleOutput,
}

impl ReferenceModelComputeAdapter {
    pub fn new(
        offer: ModelComputeOffer,
        runtime: ModelComputeRuntimeIdentity,
    ) -> Result<Self, ModelComputeRefusal> {
        let session = ModelComputeSession::discovered(offer.clone(), runtime)?;
        Ok(Self {
            offer,
            session,
            output: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            reader: PreparedNamedTensorBundleView::prepare()
                .map_err(|_| ModelComputeRefusal::UnsupportedShape)?,
        })
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
        let outputs = vec![("output".into(), 2)];
        let output = prepare_linear_output(&outputs)?;
        Ok(Self {
            offer,
            session,
            weights,
            bias,
            inputs: vec![("input".into(), 2)],
            outputs,
            output,
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
        self.output = prepare_linear_output(&self.outputs)?;
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
    fn runtime_identity(&self) -> &ModelComputeRuntimeIdentity {
        self.session.runtime()
    }
    fn load(&mut self, identity: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        load(&mut self.session, identity, bytes)
    }
    fn execute(
        &mut self,
        invocation: ModelComputeInvocation<'_>,
    ) -> ModelComputeAdapterTerminal<'_> {
        let input = match validate_invocation(&self.offer, &self.session, &invocation) {
            Ok(value) => value,
            Err(error) => return ModelComputeAdapterTerminal::Refused(error),
        };
        if let Err(error) = self.session.begin(invocation.requirement, 0) {
            return ModelComputeAdapterTerminal::Refused(error);
        }
        if invocation.inputs.tensor_bytes() > invocation.requirement.output_bytes {
            let _ = self.session.finish();
            return ModelComputeAdapterTerminal::Refused(
                ModelComputeRefusal::ResourceBoundExceeded,
            );
        }
        self.output.clear();
        self.output.extend_from_slice(invocation.inputs.encoded());
        let _ = self.session.finish();
        let outputs = self
            .reader
            .read(&self.output)
            .expect("exact admitted input encoding")
            .encoded_ref();
        let runtime = self.session.runtime();
        ModelComputeAdapterTerminal::Produced(ModelComputeExecution {
            request_identity: invocation.request_identity,
            artifact_identity: invocation.artifact_identity,
            checkpoint_identity: None,
            inputs_identity: input,
            outputs,
            consumed_work_units: 1,
            runtime,
        })
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
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
    fn runtime_identity(&self) -> &ModelComputeRuntimeIdentity {
        self.session.runtime()
    }
    fn load(&mut self, identity: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        load(&mut self.session, identity, bytes)
    }
    fn output_template(&self) -> Result<Option<NamedTensorBundle>, ModelComputeRefusal> {
        NamedTensorBundle::from_values(
            self.outputs
                .iter()
                .map(|(name, width)| linear_output(name, *width, &[0; 8][..*width * 4]))
                .collect(),
        )
        .map(Some)
        .map_err(|_| ModelComputeRefusal::UnsupportedShape)
    }
    fn execute(
        &mut self,
        invocation: ModelComputeInvocation<'_>,
    ) -> ModelComputeAdapterTerminal<'_> {
        let inputs_identity = match validate_invocation(&self.offer, &self.session, &invocation) {
            Ok(value) => value,
            Err(error) => return ModelComputeAdapterTerminal::Refused(error),
        };
        if invocation.inputs.values().len() != self.inputs.len() {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        }
        let mut values = [0_f32; 2];
        let mut offset = 0;
        for (name, width) in &self.inputs {
            let Some(named) = invocation.inputs.values().find(|v| v.name == name) else {
                return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
            };
            let input = named;
            if input.element != TensorElement::F32
                || input.dimensions() != [*width as u64]
                || (0..input.dimensions().len()).any(|index| {
                    !input
                        .axis_role(index)
                        .is_ok_and(|role| role.matches(&conduit_data::TensorAxisRole::Feature))
                })
            {
                return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
            }
            let TensorBackingView::Inline(bytes) = input.backing else {
                return ModelComputeAdapterTerminal::Refused(
                    ModelComputeRefusal::UnsupportedFormat,
                );
            };
            for bytes in bytes.as_chunks::<4>().0 {
                values[offset] = f32::from_le_bytes(*bytes);
                offset += 1;
            }
        }
        if let Err(error) = self.session.begin(invocation.requirement, 0) {
            return ModelComputeAdapterTerminal::Refused(error);
        }
        let result = [
            self.weights[0][0] * values[0] + self.weights[0][1] * values[1] + self.bias[0],
            self.weights[1][0] * values[0] + self.weights[1][1] * values[1] + self.bias[1],
        ];
        let mut bytes = [0_u8; 8];
        for (index, value) in result.iter().enumerate() {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        self.output.reset();
        let mut offset = 0;
        for (name, width) in &self.outputs {
            if self
                .output
                .write(name, &bytes[offset * 4..(offset + width) * 4])
                .is_err()
            {
                let _ = self.session.finish();
                return ModelComputeAdapterTerminal::Refused(
                    ModelComputeRefusal::ResourceBoundExceeded,
                );
            }
            offset += width;
        }
        let _ = self.session.finish();
        let Ok(outputs) = self.output.view() else {
            return ModelComputeAdapterTerminal::Failed;
        };
        if outputs.tensor_bytes() > invocation.requirement.output_bytes {
            return ModelComputeAdapterTerminal::Refused(
                ModelComputeRefusal::ResourceBoundExceeded,
            );
        }
        ModelComputeAdapterTerminal::Produced(ModelComputeExecution {
            request_identity: invocation.request_identity,
            artifact_identity: invocation.artifact_identity,
            checkpoint_identity: None,
            inputs_identity,
            outputs: outputs.encoded_ref(),
            consumed_work_units: 8,
            runtime: self.session.runtime(),
        })
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
        self.session.cancel()?;
        Ok(ModelComputeAdapterTerminal::Cancelled)
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        unload(&mut self.session)
    }
}

fn prepare_linear_output(
    ports: &[(String, usize)],
) -> Result<PreparedNamedTensorBundleOutput, ModelComputeRefusal> {
    let template = NamedTensorBundle::from_values(
        ports
            .iter()
            .map(|(name, width)| linear_output(name, *width, &[0; 8][..*width * 4]))
            .collect(),
    )
    .map_err(|_| ModelComputeRefusal::UnsupportedShape)?;
    PreparedNamedTensorBundleOutput::prepare(&template)
        .map_err(|_| ModelComputeRefusal::UnsupportedShape)
}

fn linear_output(name: &str, width: usize, bytes: &[u8]) -> NamedTensor {
    NamedTensor {
        name: name.into(),
        value: TensorValue {
            element: TensorElement::F32,
            dimensions: conduit_plot::rust_binding::BoundedSequence::try_from_iter([width as u64])
                .expect("one dimension"),
            axes: conduit_plot::rust_binding::BoundedSequence::try_from_iter([
                conduit_data::TensorAxis {
                    role: conduit_data::TensorAxisRole::Feature,
                    identity: None,
                    unit: None,
                },
            ])
            .expect("one axis"),
            content_digest: tensor_content_digest(bytes),
            backing: TensorBacking::Inline(BoundedBytes::new(bytes).expect("two f32 values fit")),
        },
    }
}

fn validate_invocation(
    offer: &ModelComputeOffer,
    session: &ModelComputeSession,
    invocation: &ModelComputeInvocation<'_>,
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
    offer.admits(invocation.requirement)?;

    for input in invocation.inputs.values() {
        if !invocation.requirement.elements.contains(&input.element) {
            return Err(ModelComputeRefusal::UnsupportedElement);
        }
        if input.dimensions().len() > usize::from(invocation.requirement.rank) {
            return Err(ModelComputeRefusal::UnsupportedShape);
        }
    }
    if invocation.inputs.tensor_bytes() != invocation.requirement.input_bytes {
        return Err(ModelComputeRefusal::ResourceBoundExceeded);
    }
    Ok(invocation.inputs.semantic_digest())
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
