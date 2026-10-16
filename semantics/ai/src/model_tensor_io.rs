//! Validate canonical named tensor values against the existing semantic signature.
use crate::{
    ModelDimensionConstraint, ModelPortConstraint, ModelPortPresence, ModelSignature,
    ModelValueConstraint,
};
use conduit_data::{
    NamedTensorBundle, NamedTensorBundleRefusal, NamedTensorBundleView, NamedTensorView,
    TensorAxisRole, TensorElement, TensorValue,
};

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
        inputs.validate().map_err(ModelTensorIoRefusal::Bundle)?;
        validate_ports(self.inputs.get().as_slice(), TensorValues::Owned(inputs))
    }

    pub fn validate_named_outputs(
        &self,
        outputs: &NamedTensorBundle,
    ) -> Result<(), ModelTensorIoRefusal> {
        self.validate()
            .map_err(|_| ModelTensorIoRefusal::Signature)?;
        outputs.validate().map_err(ModelTensorIoRefusal::Bundle)?;
        validate_ports(self.outputs.get().as_slice(), TensorValues::Owned(outputs))
    }
}

/// Signature laws are checked once before Play. Input views retain the Data
/// owner's exact canonical schema and validation; they never open resources.
pub struct PreparedModelTensorIo {
    signature: ModelSignature,
}
impl PreparedModelTensorIo {
    pub fn prepare(signature: &ModelSignature) -> Result<Self, ModelTensorIoRefusal> {
        signature
            .validate()
            .map_err(|_| ModelTensorIoRefusal::Signature)?;
        Ok(Self {
            signature: signature.clone(),
        })
    }
    pub fn signature(&self) -> &ModelSignature {
        &self.signature
    }
    pub fn validate_inputs(
        &self,
        values: &NamedTensorBundleView<'_>,
    ) -> Result<(), ModelTensorIoRefusal> {
        validate_ports(
            self.signature.inputs.get().as_slice(),
            TensorValues::Borrowed(values),
        )
    }
    pub fn validate_outputs(
        &self,
        values: &NamedTensorBundleView<'_>,
    ) -> Result<(), ModelTensorIoRefusal> {
        validate_ports(
            self.signature.outputs.get().as_slice(),
            TensorValues::Borrowed(values),
        )
    }
    pub fn validate_owned_outputs(
        &self,
        values: &NamedTensorBundle,
    ) -> Result<(), ModelTensorIoRefusal> {
        values.validate().map_err(ModelTensorIoRefusal::Bundle)?;
        validate_ports(
            self.signature.outputs.get().as_slice(),
            TensorValues::Owned(values),
        )
    }
}

pub(crate) enum TensorValues<'borrow, 'data> {
    Owned(&'borrow NamedTensorBundle),
    Borrowed(&'borrow NamedTensorBundleView<'data>),
}
enum Tensor<'borrow, 'data> {
    Owned(&'borrow TensorValue),
    Borrowed(&'borrow NamedTensorView<'data>),
}
impl<'borrow, 'data> TensorValues<'borrow, 'data> {
    fn unknown_port(&self, ports: &[ModelPortConstraint]) -> bool {
        let known = |name: &str| ports.iter().any(|port| port.identity.get() == name);
        match self {
            Self::Owned(values) => values.get().iter().any(|value| !known(&value.name)),
            Self::Borrowed(values) => values.values().any(|value| !known(value.name)),
        }
    }
    fn find(&self, name: &str) -> Option<Tensor<'borrow, 'data>> {
        match self {
            Self::Owned(values) => values
                .get()
                .iter()
                .find(|value| value.name == name)
                .map(|value| Tensor::Owned(&value.value)),
            Self::Borrowed(values) => values
                .values()
                .find(|value| value.name == name)
                .map(Tensor::Borrowed),
        }
    }
}
impl Tensor<'_, '_> {
    fn element(&self) -> TensorElement {
        match self {
            Self::Owned(value) => value.element,
            Self::Borrowed(value) => value.element,
        }
    }
    fn dimensions(&self) -> &[u64] {
        match self {
            Self::Owned(value) => value.dimensions.as_slice(),
            Self::Borrowed(value) => value.dimensions(),
        }
    }
    fn axis_matches(&self, index: usize, role: &TensorAxisRole) -> bool {
        match self {
            Self::Owned(value) => value
                .axes
                .as_slice()
                .get(index)
                .is_some_and(|axis| &axis.role == role),
            Self::Borrowed(value) => value
                .axis_role(index)
                .is_ok_and(|actual| actual.matches(role)),
        }
    }
    fn byte_count(&self) -> Result<u64, ModelTensorIoRefusal> {
        match self {
            Self::Owned(value) => value
                .byte_count()
                .map_err(|_| ModelTensorIoRefusal::ByteBound),
            Self::Borrowed(value) => Ok(value.byte_count()),
        }
    }
}

pub(crate) fn validate_ports(
    ports: &[ModelPortConstraint],
    values: TensorValues<'_, '_>,
) -> Result<(), ModelTensorIoRefusal> {
    use ModelTensorIoRefusal as R;
    if values.unknown_port(ports) {
        return Err(R::UnknownPort);
    }
    for port in ports {
        let Some(tensor) = values.find(port.identity.get()) else {
            if port.presence == ModelPortPresence::Required {
                return Err(R::MissingPort);
            }
            continue;
        };
        let constraint = match &port.value {
            ModelValueConstraint::Tensor(value) => value.constraint(),
            ModelValueConstraint::SampledSignal(value) => value.constraint(),
            ModelValueConstraint::ProbabilisticTensor(value) => value.constraint(),
            ModelValueConstraint::ProbabilisticSignal(value) => value.constraint(),
        };
        if !constraint
            .elements
            .get()
            .as_slice()
            .contains(&tensor.element())
        {
            return Err(R::Dtype);
        }
        let axes = constraint.axes.get().as_slice();
        if axes.len() != tensor.dimensions().len() {
            return Err(R::Rank);
        }
        for (index, (axis, dimension)) in axes.iter().zip(tensor.dimensions()).enumerate() {
            let valid = match &axis.dimension {
                ModelDimensionConstraint::Fixed(value) => dimension == value.value(),
                ModelDimensionConstraint::Bounded(value) => {
                    dimension >= value.minimum() && dimension <= value.maximum()
                }
            };
            if !valid {
                return Err(R::Dimension);
            }
            if !tensor.axis_matches(index, &axis.role) {
                return Err(R::AxisRole);
            }
        }
        if tensor.byte_count()? > constraint.maximum_bytes {
            return Err(R::ByteBound);
        }
    }
    Ok(())
}
