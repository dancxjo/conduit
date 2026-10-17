//! Host-local authoring seam; semantic tensor values cross the public boundary.
use burn::{
    module::{Module, ParamGroup},
    tensor::{Device, Tensor},
};
use conduit_ai::{
    ModelDimensionConstraint, ModelPortConstraint, ModelValueConstraint, TrainingMetric,
    TrainingObjective,
};
use conduit_data::{TensorBacking, TensorElement, TensorValue};

use crate::{AuthoringDescriptor, Error};

#[derive(Debug, Clone)]
pub struct ModelBatch {
    pub inputs: Vec<TensorValue>,
    pub targets: Vec<TensorValue>,
}

pub struct ObjectiveResult {
    pub loss: Tensor<1>,
    pub metrics: Vec<TrainingMetric>,
}

/// Compiled Rust model family. Implementations are trusted cooperative code.
/// The adapter owns admission, state publication and durable checkpointing.
pub trait BurnModelDefinition {
    type Model: Module;
    fn descriptor(&self) -> AuthoringDescriptor;
    fn initialize(&self, device: &Device) -> Result<Self::Model, Error>;
    fn parameter_group(&self, model: &Self::Model, identity: &str) -> Result<ParamGroup, Error>;
    fn forward(
        &self,
        model: &Self::Model,
        inputs: &[TensorValue],
        device: &Device,
    ) -> Result<Vec<TensorValue>, Error>;
    fn objective(
        &self,
        model: &Self::Model,
        batch: &ModelBatch,
        objectives: &[TrainingObjective],
        device: &Device,
    ) -> Result<ObjectiveResult, Error>;
}

pub(crate) fn validate_values(
    values: &[TensorValue],
    ports: &[ModelPortConstraint],
    maximum_bytes: u64,
) -> Result<u64, Error> {
    if values.len() != ports.len() {
        return Err(Error::InvalidTensor);
    }
    let mut total = 0u64;
    for (value, port) in values.iter().zip(ports) {
        value.validate().map_err(|_| Error::InvalidTensor)?;
        let constraint = match &port.value {
            ModelValueConstraint::Tensor(v) => v.constraint(),
            ModelValueConstraint::SampledSignal(v) => v.constraint(),
            _ => return Err(Error::InvalidTensor),
        };
        let bytes = value.byte_count().map_err(|_| Error::InvalidTensor)?;
        total = total.checked_add(bytes).ok_or(Error::ResourceBound)?;
        if !constraint.elements.get().contains(&value.element)
            || constraint.axes.get().len() != value.dimensions.len()
            || bytes > *constraint.maximum_bytes()
        {
            return Err(Error::InvalidTensor);
        }
        for ((dimension, role), axis) in value
            .dimensions
            .iter()
            .zip(&value.axes)
            .zip(constraint.axes.get().iter())
        {
            if role.role != axis.role {
                return Err(Error::InvalidTensor);
            }
            let admitted = match &axis.dimension {
                ModelDimensionConstraint::Fixed(v) => *dimension == *v.value(),
                ModelDimensionConstraint::Bounded(v) => {
                    *dimension >= *v.minimum() && *dimension <= *v.maximum()
                }
            };
            if !admitted {
                return Err(Error::InvalidTensor);
            }
        }
        if value.element == TensorElement::F32 {
            let TensorBacking::Inline(data) = &value.backing else {
                return Err(Error::InvalidTensor);
            };
            if data
                .as_slice()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|b| !f32::from_le_bytes(*b).is_finite())
            {
                return Err(Error::NumericFailure);
            }
        }
    }
    if total > maximum_bytes {
        return Err(Error::ResourceBound);
    }
    Ok(total)
}
