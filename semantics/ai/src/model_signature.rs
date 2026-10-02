//! Finite provider-neutral callable model signatures.

use alloc::vec::Vec;
use conduit_core::semantic_digest;
use conduit_data::TensorAxisRole;
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};

use crate::{
    ModelDimensionConstraint, ModelOperation, ModelOperationForm, ModelOperations,
    ModelPortConstraint, ModelPortIdentity, ModelPortPresence, ModelPortPresenceForm, ModelPorts,
    ModelSemanticKind, ModelSignature, ModelSignatureRefusal, ModelTensorAxes,
    ModelTensorConstraint, ModelTensorElements, ModelValueConstraint,
};

pub const MODEL_SIGNATURE_INFO_ID: &str = "model/signature@1";
pub const MAXIMUM_MODEL_PORTS: usize = 32;
pub const MAXIMUM_MODEL_OPERATIONS: usize = 8;
pub const MAXIMUM_MODEL_ELEMENTS: usize = 8;
pub const MAXIMUM_MODEL_RANK: usize = 8;
pub const MAXIMUM_MODEL_IDENTITY_BYTES: usize = 128;

impl ModelSignature {
    pub fn from_parts(
        identity: alloc::string::String,
        compatibility_version: u32,
        operations: Vec<ModelOperation>,
        inputs: Vec<ModelPortConstraint>,
        outputs: Vec<ModelPortConstraint>,
    ) -> Result<Self, NativeBindingRefusal> {
        Self::new(
            compatibility_version,
            identity,
            ModelPorts::new(
                BoundedSequence::try_from_iter(inputs).map_err(|_| invalid_native_value())?,
            )?,
            ModelOperations::new(
                BoundedSequence::try_from_iter(operations).map_err(|_| invalid_native_value())?,
            )?,
            ModelPorts::new(
                BoundedSequence::try_from_iter(outputs).map_err(|_| invalid_native_value())?,
            )?,
        )
    }

    pub(crate) fn inference_only(
        identity: alloc::string::String,
        compatibility_version: u32,
        inputs: Vec<(
            alloc::string::String,
            alloc::string::String,
            ModelValueConstraint,
        )>,
    ) -> Result<Self, ModelSignatureRefusal> {
        let operations = bounded([ModelOperation::Infer])?;
        let operations = ModelOperations::new(operations)
            .map_err(|_| ModelSignatureRefusal::MissingOperation)?;
        let inputs = inputs
            .into_iter()
            .map(|(identity, semantic_kind, value)| {
                ModelPortConstraint::new(
                    ModelPortIdentity::new(identity)
                        .map_err(|_| ModelSignatureRefusal::InvalidIdentity)?,
                    ModelPortPresence::Optional,
                    ModelSemanticKind::new(semantic_kind)
                        .map_err(|_| ModelSignatureRefusal::InvalidIdentity)?,
                    value,
                )
                .map_err(|_| ModelSignatureRefusal::InvalidTensorConstraint)
            })
            .collect::<Result<Vec<_>, ModelSignatureRefusal>>()?;
        let inputs =
            ModelPorts::new(bounded(inputs)?).map_err(|_| ModelSignatureRefusal::TooManyPorts)?;
        let outputs = ModelPorts::new(BoundedSequence::new())
            .map_err(|_| ModelSignatureRefusal::TooManyPorts)?;
        Self::new(compatibility_version, identity, inputs, operations, outputs)
            .map_err(|_| ModelSignatureRefusal::InvalidTensorConstraint)
    }

    pub fn validate(&self) -> Result<(), ModelSignatureRefusal> {
        let operations = self.operations.get().as_slice();
        let inputs = self.inputs.get().as_slice();
        let outputs = self.outputs.get().as_slice();
        if has_duplicate(operations) {
            return Err(ModelSignatureRefusal::DuplicateOperation);
        }
        let port_count = inputs.len().saturating_add(outputs.len());
        if port_count == 0 {
            return Err(ModelSignatureRefusal::MissingPort);
        }
        if port_count > MAXIMUM_MODEL_PORTS {
            return Err(ModelSignatureRefusal::TooManyPorts);
        }
        for port in inputs.iter().chain(outputs) {
            validate_tensor(port)?;
        }
        let input_identities = inputs.iter().map(|port| &port.identity).collect::<Vec<_>>();
        let output_identities = outputs
            .iter()
            .map(|port| &port.identity)
            .collect::<Vec<_>>();
        if has_duplicate(&input_identities) || has_duplicate(&output_identities) {
            return Err(ModelSignatureRefusal::DuplicatePort);
        }
        Ok(())
    }

    pub fn semantic_digest(&self) -> Result<[u8; 32], ModelSignatureRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        push_text(&mut bytes, &self.identity);
        bytes.extend_from_slice(&self.compatibility_version.to_le_bytes());
        let operations = self.operations.get().as_slice();
        push_len(&mut bytes, operations.len());
        for operation in operations {
            bytes.push(ModelOperationForm::encode(*operation)[0]);
        }
        encode_ports(&mut bytes, self.inputs.get().as_slice(), 0);
        encode_ports(&mut bytes, self.outputs.get().as_slice(), 1);
        Ok(semantic_digest(MODEL_SIGNATURE_INFO_ID, &bytes))
    }
}

impl ModelTensorConstraint {
    pub fn from_parts(
        elements: Vec<conduit_data::TensorElement>,
        axes: Vec<crate::ModelAxisConstraint>,
        maximum_bytes: u64,
    ) -> Result<Self, NativeBindingRefusal> {
        Self::new(
            ModelTensorAxes::new(
                BoundedSequence::try_from_iter(axes).map_err(|_| invalid_native_value())?,
            )?,
            ModelTensorElements::new(
                BoundedSequence::try_from_iter(elements).map_err(|_| invalid_native_value())?,
            )?,
            maximum_bytes,
        )
    }
}

impl ModelPortConstraint {
    pub fn from_parts(
        identity: alloc::string::String,
        semantic_kind: alloc::string::String,
        presence: ModelPortPresence,
        value: ModelValueConstraint,
    ) -> Result<Self, NativeBindingRefusal> {
        Self::new(
            ModelPortIdentity::new(identity)?,
            presence,
            ModelSemanticKind::new(semantic_kind)?,
            value,
        )
    }
}

fn invalid_native_value() -> NativeBindingRefusal {
    NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongCollectionLength)
}

fn bounded<T, const MAXIMUM: usize>(
    values: impl IntoIterator<Item = T>,
) -> Result<BoundedSequence<T, MAXIMUM>, ModelSignatureRefusal> {
    BoundedSequence::try_from_iter(values).map_err(|_| ModelSignatureRefusal::TooManyPorts)
}

fn validate_tensor(port: &ModelPortConstraint) -> Result<(), ModelSignatureRefusal> {
    let (tensor, signal) = match &port.value {
        ModelValueConstraint::Tensor(value) => (value.constraint(), false),
        ModelValueConstraint::SampledSignal(value) => (value.constraint(), true),
        ModelValueConstraint::ProbabilisticTensor(value) => (value.constraint(), false),
        ModelValueConstraint::ProbabilisticSignal(value) => (value.constraint(), true),
    };
    let elements = tensor.elements.get().as_slice();
    let axes = tensor.axes.get().as_slice();
    if has_duplicate(elements)
        || axes.iter().any(|axis| match &axis.dimension {
            ModelDimensionConstraint::Fixed(value) => *value.value() == 0,
            ModelDimensionConstraint::Bounded(value) => value.minimum() > value.maximum(),
        })
    {
        return Err(ModelSignatureRefusal::InvalidTensorConstraint);
    }
    let maximum_elements = axes.iter().try_fold(1_u64, |count, axis| {
        let dimension = match &axis.dimension {
            ModelDimensionConstraint::Fixed(value) => *value.value(),
            ModelDimensionConstraint::Bounded(value) => *value.maximum(),
        };
        count.checked_mul(dimension)
    });
    let largest_element = tensor
        .elements
        .get()
        .iter()
        .map(|element| element.byte_width())
        .max()
        .unwrap_or(0);
    if maximum_elements
        .and_then(|count| count.checked_mul(largest_element))
        .is_none_or(|bytes| bytes > tensor.maximum_bytes)
    {
        return Err(ModelSignatureRefusal::InvalidTensorConstraint);
    }
    if signal && axes.first().map(|axis| &axis.role) != Some(&TensorAxisRole::Time) {
        return Err(ModelSignatureRefusal::InvalidSignalConstraint);
    }
    Ok(())
}

fn encode_ports(output: &mut Vec<u8>, ports: &[ModelPortConstraint], direction: u8) {
    output.push(direction);
    push_len(output, ports.len());
    for port in ports {
        push_text(output, port.identity.get());
        push_text(output, port.semantic_kind.get());
        output.push(ModelPortPresenceForm::encode(port.presence)[0]);
        let tensor = match &port.value {
            ModelValueConstraint::Tensor(value) => {
                output.push(0);
                value.constraint()
            }
            ModelValueConstraint::SampledSignal(value) => {
                output.push(1);
                value.constraint()
            }
            ModelValueConstraint::ProbabilisticTensor(value) => {
                output.push(2);
                value.constraint()
            }
            ModelValueConstraint::ProbabilisticSignal(value) => {
                output.push(3);
                value.constraint()
            }
        };
        push_len(output, tensor.elements.get().len());
        for element in tensor.elements.get() {
            push_text(output, element.semantic_id());
        }
        push_len(output, tensor.axes.get().len());
        for axis in tensor.axes.get() {
            encode_axis_role(output, &axis.role);
            push_dimension_constraint(output, &axis.dimension);
        }
        output.extend_from_slice(&tensor.maximum_bytes.to_le_bytes());
    }
}

fn push_dimension_constraint(output: &mut Vec<u8>, value: &ModelDimensionConstraint) {
    match value {
        ModelDimensionConstraint::Fixed(value) => {
            output.push(0);
            output.extend_from_slice(&value.value().to_le_bytes());
        }
        ModelDimensionConstraint::Bounded(value) => {
            output.push(1);
            output.extend_from_slice(&value.minimum().to_le_bytes());
            output.extend_from_slice(&value.maximum().to_le_bytes());
        }
    }
}

fn encode_axis_role(output: &mut Vec<u8>, role: &TensorAxisRole) {
    match role {
        TensorAxisRole::Batch => output.push(0),
        TensorAxisRole::Time => output.push(1),
        TensorAxisRole::Feature => output.push(2),
        TensorAxisRole::Sensor => output.push(3),
        TensorAxisRole::SpatialCoordinate => output.push(4),
        TensorAxisRole::Frequency => output.push(5),
        TensorAxisRole::Channel => output.push(6),
        TensorAxisRole::Other(value) => {
            output.push(7);
            push_text(output, value.identity());
        }
    }
}

fn has_duplicate<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}

fn push_len(output: &mut Vec<u8>, value: usize) {
    output.extend_from_slice(&(value as u16).to_le_bytes());
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    push_len(output, value.len());
    output.extend_from_slice(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_constraint_retains_the_v1_manual_digest_bytes() {
        let mut bytes = Vec::new();
        push_dimension_constraint(
            &mut bytes,
            &ModelDimensionConstraint::fixed(0x0102_0304_0506_0708).unwrap(),
        );
        assert_eq!(bytes, [0, 8, 7, 6, 5, 4, 3, 2, 1]);

        bytes.clear();
        push_dimension_constraint(
            &mut bytes,
            &ModelDimensionConstraint::bounded(0x1112_1314_1516_1718, 0x0102_0304_0506_0708)
                .unwrap(),
        );
        assert_eq!(
            bytes,
            [1, 8, 7, 6, 5, 4, 3, 2, 1, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11,]
        );
    }
}
