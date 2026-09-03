//! Finite provider-neutral callable model signatures.

use alloc::{string::String, vec::Vec};
use conduit_core::semantic_digest;
use conduit_data::{TensorAxisRole, TensorElement};

pub const MODEL_SIGNATURE_INFO_ID: &str = "model/signature@1";
pub const MAXIMUM_MODEL_PORTS: usize = 32;
pub const MAXIMUM_MODEL_OPERATIONS: usize = 8;
pub const MAXIMUM_MODEL_ELEMENTS: usize = 8;
pub const MAXIMUM_MODEL_RANK: usize = 8;
pub const MAXIMUM_MODEL_IDENTITY_BYTES: usize = 128;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelOperation {
    Infer,
    Encode,
    Decode,
    Sample,
    LogProbability,
    Evaluate,
    Train,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelPortPresence {
    Required,
    Optional,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelDimensionConstraint {
    Fixed(u64),
    Bounded { minimum: u64, maximum: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelAxisConstraint {
    pub role: TensorAxisRole,
    pub dimension: ModelDimensionConstraint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTensorConstraint {
    pub elements: Vec<TensorElement>,
    pub axes: Vec<ModelAxisConstraint>,
    pub maximum_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelValueConstraint {
    Tensor(ModelTensorConstraint),
    SampledSignal(ModelTensorConstraint),
    ProbabilisticTensor(ModelTensorConstraint),
    ProbabilisticSignal(ModelTensorConstraint),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPortConstraint {
    pub identity: String,
    pub semantic_kind: String,
    pub presence: ModelPortPresence,
    pub value: ModelValueConstraint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSignature {
    pub identity: String,
    pub compatibility_version: u32,
    pub operations: Vec<ModelOperation>,
    pub inputs: Vec<ModelPortConstraint>,
    pub outputs: Vec<ModelPortConstraint>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelSignatureRefusal {
    InvalidIdentity,
    InvalidCompatibilityVersion,
    MissingOperation,
    TooManyOperations,
    DuplicateOperation,
    MissingPort,
    TooManyPorts,
    DuplicatePort,
    InvalidTensorConstraint,
    InvalidSignalConstraint,
}

impl ModelSignature {
    pub fn validate(&self) -> Result<(), ModelSignatureRefusal> {
        validate_identity(&self.identity)?;
        if self.compatibility_version == 0 {
            return Err(ModelSignatureRefusal::InvalidCompatibilityVersion);
        }
        if self.operations.is_empty() {
            return Err(ModelSignatureRefusal::MissingOperation);
        }
        if self.operations.len() > MAXIMUM_MODEL_OPERATIONS {
            return Err(ModelSignatureRefusal::TooManyOperations);
        }
        if has_duplicate(&self.operations) {
            return Err(ModelSignatureRefusal::DuplicateOperation);
        }
        let port_count = self.inputs.len().saturating_add(self.outputs.len());
        if port_count == 0 {
            return Err(ModelSignatureRefusal::MissingPort);
        }
        if port_count > MAXIMUM_MODEL_PORTS {
            return Err(ModelSignatureRefusal::TooManyPorts);
        }
        for port in self.inputs.iter().chain(&self.outputs) {
            validate_identity(&port.identity)?;
            validate_identity(&port.semantic_kind)?;
            validate_tensor(port)?;
        }
        let input_identities = self
            .inputs
            .iter()
            .map(|port| &port.identity)
            .collect::<Vec<_>>();
        let output_identities = self
            .outputs
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
        push_len(&mut bytes, self.operations.len());
        for operation in &self.operations {
            bytes.push(operation_tag(*operation));
        }
        encode_ports(&mut bytes, &self.inputs, 0);
        encode_ports(&mut bytes, &self.outputs, 1);
        Ok(semantic_digest(MODEL_SIGNATURE_INFO_ID, &bytes))
    }
}

fn validate_tensor(port: &ModelPortConstraint) -> Result<(), ModelSignatureRefusal> {
    let (tensor, signal) = match &port.value {
        ModelValueConstraint::Tensor(tensor) => (tensor, false),
        ModelValueConstraint::SampledSignal(tensor) => (tensor, true),
        ModelValueConstraint::ProbabilisticTensor(tensor) => (tensor, false),
        ModelValueConstraint::ProbabilisticSignal(tensor) => (tensor, true),
    };
    if tensor.elements.is_empty()
        || tensor.elements.len() > MAXIMUM_MODEL_ELEMENTS
        || has_duplicate(&tensor.elements)
        || tensor.axes.is_empty()
        || tensor.axes.len() > MAXIMUM_MODEL_RANK
        || tensor.maximum_bytes == 0
        || tensor.axes.iter().any(|axis| match axis.dimension {
            ModelDimensionConstraint::Fixed(value) => value == 0,
            ModelDimensionConstraint::Bounded { minimum, maximum } => {
                minimum == 0 || minimum > maximum
            }
        })
        || tensor.axes.iter().any(|axis| {
            matches!(
                &axis.role,
                TensorAxisRole::Other(value)
                    if value.is_empty() || value.len() > MAXIMUM_MODEL_IDENTITY_BYTES
            )
        })
    {
        return Err(ModelSignatureRefusal::InvalidTensorConstraint);
    }
    let maximum_elements = tensor.axes.iter().try_fold(1_u64, |count, axis| {
        let dimension = match axis.dimension {
            ModelDimensionConstraint::Fixed(value) => value,
            ModelDimensionConstraint::Bounded { maximum, .. } => maximum,
        };
        count.checked_mul(dimension)
    });
    let largest_element = tensor
        .elements
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
    if signal && tensor.axes.first().map(|axis| &axis.role) != Some(&TensorAxisRole::Time) {
        return Err(ModelSignatureRefusal::InvalidSignalConstraint);
    }
    Ok(())
}

fn encode_ports(output: &mut Vec<u8>, ports: &[ModelPortConstraint], direction: u8) {
    output.push(direction);
    push_len(output, ports.len());
    for port in ports {
        push_text(output, &port.identity);
        push_text(output, &port.semantic_kind);
        output.push(match port.presence {
            ModelPortPresence::Required => 0,
            ModelPortPresence::Optional => 1,
        });
        let tensor = match &port.value {
            ModelValueConstraint::Tensor(value) => {
                output.push(0);
                value
            }
            ModelValueConstraint::SampledSignal(value) => {
                output.push(1);
                value
            }
            ModelValueConstraint::ProbabilisticTensor(value) => {
                output.push(2);
                value
            }
            ModelValueConstraint::ProbabilisticSignal(value) => {
                output.push(3);
                value
            }
        };
        push_len(output, tensor.elements.len());
        for element in &tensor.elements {
            push_text(output, element.semantic_id());
        }
        push_len(output, tensor.axes.len());
        for axis in &tensor.axes {
            encode_axis_role(output, &axis.role);
            match axis.dimension {
                ModelDimensionConstraint::Fixed(value) => {
                    output.push(0);
                    output.extend_from_slice(&value.to_le_bytes());
                }
                ModelDimensionConstraint::Bounded { minimum, maximum } => {
                    output.push(1);
                    output.extend_from_slice(&minimum.to_le_bytes());
                    output.extend_from_slice(&maximum.to_le_bytes());
                }
            }
        }
        output.extend_from_slice(&tensor.maximum_bytes.to_le_bytes());
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
            push_text(output, value);
        }
    }
}

fn operation_tag(operation: ModelOperation) -> u8 {
    match operation {
        ModelOperation::Infer => 0,
        ModelOperation::Encode => 1,
        ModelOperation::Decode => 2,
        ModelOperation::Sample => 3,
        ModelOperation::LogProbability => 4,
        ModelOperation::Evaluate => 5,
        ModelOperation::Train => 6,
    }
}

fn validate_identity(value: &str) -> Result<(), ModelSignatureRefusal> {
    if value.is_empty() || value.len() > MAXIMUM_MODEL_IDENTITY_BYTES {
        Err(ModelSignatureRefusal::InvalidIdentity)
    } else {
        Ok(())
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
