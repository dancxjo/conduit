//! Validate canonical named tensor values against the existing semantic signature.
use crate::{
    ModelDimensionConstraint, ModelPortConstraint, ModelPortPresence, ModelSignature,
    ModelValueConstraint,
};
use conduit_data::{NamedTensorBundle, NamedTensorBundleRefusal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelTensorIoRefusal {
    Signature,
    Bundle(NamedTensorBundleRefusal),
    UnknownPort,
    MissingPort,
    Dtype,
    Rank,
    Dimension,
    AxisRole,
    ByteBound,
}

impl ModelSignature {
    pub fn validate_named_inputs(
        &self,
        inputs: &NamedTensorBundle,
    ) -> Result<(), ModelTensorIoRefusal> {
        self.validate()
            .map_err(|_| ModelTensorIoRefusal::Signature)?;
        validate_ports(self.inputs.get().as_slice(), inputs)
    }

    pub fn validate_named_outputs(
        &self,
        outputs: &NamedTensorBundle,
    ) -> Result<(), ModelTensorIoRefusal> {
        self.validate()
            .map_err(|_| ModelTensorIoRefusal::Signature)?;
        validate_ports(self.outputs.get().as_slice(), outputs)
    }
}

fn validate_ports(
    ports: &[ModelPortConstraint],
    values: &NamedTensorBundle,
) -> Result<(), ModelTensorIoRefusal> {
    use ModelTensorIoRefusal as R;
    values.validate().map_err(R::Bundle)?;
    let values = values.get().as_slice();
    if values
        .iter()
        .any(|v| !ports.iter().any(|p| p.identity.get() == &v.name))
    {
        return Err(R::UnknownPort);
    }
    for port in ports {
        let Some(named) = values.iter().find(|v| port.identity.get() == &v.name) else {
            if port.presence == ModelPortPresence::Required {
                return Err(R::MissingPort);
            }
            continue;
        };
        let constraint = match &port.value {
            ModelValueConstraint::Tensor(v) => v.constraint(),
            ModelValueConstraint::SampledSignal(v) => v.constraint(),
            ModelValueConstraint::ProbabilisticTensor(v) => v.constraint(),
            ModelValueConstraint::ProbabilisticSignal(v) => v.constraint(),
        };
        let tensor = &named.value;
        if !constraint
            .elements
            .get()
            .as_slice()
            .contains(&tensor.element)
        {
            return Err(R::Dtype);
        }
        let axes = constraint.axes.get().as_slice();
        if axes.len() != tensor.dimensions.len() {
            return Err(R::Rank);
        }
        for ((axis, dimension), actual_axis) in axes
            .iter()
            .zip(tensor.dimensions.iter())
            .zip(tensor.axes.iter())
        {
            let valid = match &axis.dimension {
                ModelDimensionConstraint::Fixed(v) => dimension == v.value(),
                ModelDimensionConstraint::Bounded(v) => {
                    dimension >= v.minimum() && dimension <= v.maximum()
                }
            };
            if !valid {
                return Err(R::Dimension);
            }
            if actual_axis.role != axis.role {
                return Err(R::AxisRole);
            }
        }
        if tensor.byte_count().map_err(|_| R::ByteBound)? > constraint.maximum_bytes {
            return Err(R::ByteBound);
        }
    }
    Ok(())
}
