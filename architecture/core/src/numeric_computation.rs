//! Ordered numerical computation custody, independent of denotation normalization.
//!
//! This graph records each operand and operation a realization must account for.
//! Equal final mathematical values do not imply equal floating-point computations.
use alloc::{string::String, vec::Vec};
mod codec;
mod configuration;
mod evaluate;
mod ieee;
mod signature;
pub use configuration::numeric_computation_info_type;
pub use evaluate::*;
pub use ieee::ieee_rounded_literal;

use crate::{semantic_digest, MathValue, MathValueRef, NumericRefusal, Unit};

pub const MAX_NUMERIC_COMPUTATION_NODES: usize = 128;
pub const MAX_NUMERIC_COMPUTATION_DEPTH: usize = 32;
pub const MAX_NUMERIC_COMPUTATION_BYTES: usize = 65_536;
pub const NUMERIC_COMPUTATION_INFO_ID: &str = "computation/mathematical-number@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NumericOperation {
    Negate,
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Root,
    SquareRoot,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    CyclicToAngularFrequency,
    AngularToCyclicFrequency,
    QuantityConstruction,
}
impl NumericOperation {
    pub const fn arity(self) -> usize {
        match self {
            Self::Add
            | Self::Subtract
            | Self::Multiply
            | Self::Divide
            | Self::Power
            | Self::Root
            | Self::Atan2
            | Self::Equal
            | Self::NotEqual
            | Self::Less
            | Self::LessOrEqual
            | Self::Greater
            | Self::GreaterOrEqual => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericComputationOperation {
    /// Canonical exact denotation of the authored literal, before any projection.
    Literal(Vec<u8>),
    /// Explicit IEEE operational literal, preserving signed zero and NaN payload bits.
    IeeeLiteral {
        bits: u64,
    },
    /// Explicit nearest-even conversion of an exact decimal literal to IEEE.
    IeeeRoundedLiteral {
        source: Vec<u8>,
        bits: u64,
        flags: crate::numeric_realization::Binary64Flags,
    },
    Parameter(u16),
    Apply {
        operation: NumericOperation,
        operands: Vec<u16>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericComputationNode {
    /// Checked semantic family/role identity, never a machine representation.
    pub semantic_kind: String,
    /// Captured source-bound coordinate law, when the node is unit-bearing.
    pub unit: Option<alloc::boxed::Box<Unit>>,
    /// Captured canonical reference law used by an angle operation.
    pub reference_unit: Option<alloc::boxed::Box<Unit>>,
    pub operation: NumericComputationOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericComputation {
    nodes: Vec<NumericComputationNode>,
    root: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericComputationRefusal {
    NodeBudget,
    DepthBudget,
    StorageBudget,
    InvalidRoot,
    InvalidOperand,
    WrongArity,
    InvalidSemanticKind,
    UnreachableNode,
    MalformedEncoding,
    InvalidUnit,
    InvalidLiteral(NumericRefusal),
}

impl NumericComputationNode {
    pub fn literal(
        semantic_kind: String,
        unit: Option<alloc::boxed::Box<Unit>>,
        value: &MathValue,
    ) -> Result<Self, NumericComputationRefusal> {
        Ok(Self {
            semantic_kind,
            unit,
            reference_unit: None,
            operation: NumericComputationOperation::Literal(
                value
                    .canonical_bytes()
                    .map_err(NumericComputationRefusal::InvalidLiteral)?,
            ),
        })
    }
}

impl NumericComputation {
    /// Retained owned storage, including actual allocated capacities. This
    /// inspection does not allocate or serialize the computation.
    pub fn owned_storage_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
            + self.nodes.capacity() * core::mem::size_of::<NumericComputationNode>()
            + self.nodes.iter().map(|node| {
                node.semantic_kind.capacity()
                    + node.unit.as_ref().map_or(0, |unit| unit.owned_storage_bytes())
                    + node.reference_unit.as_ref().map_or(0, |unit| unit.owned_storage_bytes())
                    + match &node.operation {
                        NumericComputationOperation::Literal(bytes) => bytes.capacity(),
                        NumericComputationOperation::IeeeRoundedLiteral { source, .. } => source.capacity(),
                        NumericComputationOperation::Apply { operands, .. } => operands.capacity() * core::mem::size_of::<u16>(),
                        NumericComputationOperation::Parameter(_) | NumericComputationOperation::IeeeLiteral { .. } => 0,
                    }
            }).sum::<usize>()
    }
    pub fn new(
        nodes: Vec<NumericComputationNode>,
        root: u16,
    ) -> Result<Self, NumericComputationRefusal> {
        let value = Self { nodes, root };
        value.validate()?;
        Ok(value)
    }
    pub fn nodes(&self) -> &[NumericComputationNode] {
        &self.nodes
    }
    pub const fn root(&self) -> u16 {
        self.root
    }

    pub fn validate(&self) -> Result<(), NumericComputationRefusal> {
        if self.nodes.is_empty() || self.nodes.len() > MAX_NUMERIC_COMPUTATION_NODES {
            return Err(NumericComputationRefusal::NodeBudget);
        }
        if usize::from(self.root) >= self.nodes.len() {
            return Err(NumericComputationRefusal::InvalidRoot);
        }
        let mut depths = [0usize; MAX_NUMERIC_COMPUTATION_NODES];
        for (index, node) in self.nodes.iter().enumerate() {
            if node.semantic_kind.is_empty() || node.semantic_kind.len() > 256 {
                return Err(NumericComputationRefusal::InvalidSemanticKind);
            }
            if let Some(unit) = &node.unit {
                let Some((family, role)) = crate::parse_quantity_semantic_role(&node.semantic_kind)
                else {
                    return Err(NumericComputationRefusal::InvalidUnit);
                };
                if family != unit.family().identity() || !unit.family().admits(role) {
                    return Err(NumericComputationRefusal::InvalidUnit);
                }
            }
            if let Some(reference) = &node.reference_unit {
                let reference_kind = match node.operation {
                    NumericComputationOperation::Apply { operation: NumericOperation::CyclicToAngularFrequency, .. } => crate::FREQUENCY_INFO_ID,
                    NumericComputationOperation::Apply { operation: NumericOperation::AngularToCyclicFrequency, .. } => crate::ANGULAR_FREQUENCY_INFO_ID,
                    _ => crate::ANGLE_INFO_ID,
                };
                let Some((family, role)) = crate::parse_quantity_semantic_role(reference_kind)
                else {
                    return Err(NumericComputationRefusal::InvalidUnit);
                };
                if reference.family().identity() != family || !reference.family().admits(role) {
                    return Err(NumericComputationRefusal::InvalidUnit);
                }
            }
            depths[index] = 1;
            match &node.operation {
                NumericComputationOperation::Literal(bytes) => {
                    if node.reference_unit.is_some() {
                        return Err(NumericComputationRefusal::InvalidUnit);
                    }
                    MathValueRef::validate(bytes)
                        .map_err(NumericComputationRefusal::InvalidLiteral)?;
                    if let Some(profile) =
                        crate::PortableNumericProfile::from_info_id(&node.semantic_kind)
                    {
                        profile
                            .validate(bytes)
                            .map_err(NumericComputationRefusal::InvalidLiteral)?;
                    }
                    if !is_scalar_kind(&node.semantic_kind)
                        && crate::parse_quantity_semantic_role(&node.semantic_kind).is_none()
                    {
                        return Err(NumericComputationRefusal::InvalidSemanticKind);
                    }
                    if crate::parse_quantity_semantic_role(&node.semantic_kind).is_some()
                        && node.unit.is_none()
                    {
                        return Err(NumericComputationRefusal::InvalidUnit);
                    }
                }
                NumericComputationOperation::IeeeRoundedLiteral {
                    source,
                    bits,
                    flags,
                } => {
                    if node.semantic_kind != crate::F64_INFO_ID
                        || node.unit.is_some()
                        || node.reference_unit.is_some()
                    {
                        return Err(NumericComputationRefusal::InvalidSemanticKind);
                    }
                    ieee::validate(source, *bits, *flags)?;
                }
                NumericComputationOperation::IeeeLiteral { .. } => {
                    if node.semantic_kind != crate::F64_INFO_ID
                        || node.unit.is_some()
                        || node.reference_unit.is_some()
                    {
                        return Err(NumericComputationRefusal::InvalidSemanticKind);
                    }
                }
                NumericComputationOperation::Parameter(_) => {
                    if !is_scalar_kind(&node.semantic_kind)
                        && node.semantic_kind != crate::F64_INFO_ID
                        && crate::parse_quantity_semantic_role(&node.semantic_kind).is_none()
                    {
                        return Err(NumericComputationRefusal::InvalidSemanticKind);
                    }
                    if node.reference_unit.is_some() {
                        return Err(NumericComputationRefusal::InvalidUnit);
                    }
                }
                NumericComputationOperation::Apply {
                    operation,
                    operands,
                } => {
                    if operands.len() != operation.arity() {
                        return Err(NumericComputationRefusal::WrongArity);
                    }
                    for operand in operands {
                        let operand = usize::from(*operand);
                        if operand >= index {
                            return Err(NumericComputationRefusal::InvalidOperand);
                        }
                        depths[index] = depths[index].max(depths[operand] + 1);
                    }
                    let operands = operands
                        .iter()
                        .map(|operand| &self.nodes[usize::from(*operand)])
                        .collect::<Vec<_>>();
                    signature::validate(node, *operation, &operands)?;
                }
            }
            if depths[index] > MAX_NUMERIC_COMPUTATION_DEPTH {
                return Err(NumericComputationRefusal::DepthBudget);
            }
        }
        let mut reachable = [false; MAX_NUMERIC_COMPUTATION_NODES];
        reachable[usize::from(self.root)] = true;
        for index in (0..self.nodes.len()).rev() {
            if reachable[index] {
                if let NumericComputationOperation::Apply { operands, .. } =
                    &self.nodes[index].operation
                {
                    for operand in operands {
                        reachable[usize::from(*operand)] = true;
                    }
                }
            }
        }
        if reachable[..self.nodes.len()].iter().any(|value| !value) {
            return Err(NumericComputationRefusal::UnreachableNode);
        }
        self.canonical_bytes().map(|_| ())
    }

    pub fn decode_validate(bytes: &[u8]) -> Result<Self, NumericComputationRefusal> {
        codec::decode(bytes)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, NumericComputationRefusal> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"conduit.numeric-computation.v1");
        bytes.extend_from_slice(&(self.nodes.len() as u16).to_be_bytes());
        bytes.extend_from_slice(&self.root.to_be_bytes());
        for node in &self.nodes {
            bytes.extend_from_slice(&(node.semantic_kind.len() as u16).to_be_bytes());
            bytes.extend_from_slice(node.semantic_kind.as_bytes());
            match &node.unit {
                None => bytes.push(0),
                Some(unit) => {
                    let encoded = unit.encode();
                    bytes.push(1);
                    bytes.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
                    bytes.extend_from_slice(&encoded);
                }
            }
            match &node.reference_unit {
                None => bytes.push(0),
                Some(unit) => {
                    let encoded = unit.encode();
                    bytes.push(1);
                    bytes.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
                    bytes.extend_from_slice(&encoded);
                }
            }
            match &node.operation {
                NumericComputationOperation::Literal(value) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&(value.len() as u32).to_be_bytes());
                    bytes.extend_from_slice(value);
                }
                NumericComputationOperation::IeeeLiteral { bits } => {
                    bytes.push(3);
                    bytes.extend_from_slice(&bits.to_be_bytes());
                }
                NumericComputationOperation::IeeeRoundedLiteral {
                    source,
                    bits,
                    flags,
                } => {
                    bytes.push(4);
                    bytes.extend_from_slice(&(source.len() as u32).to_be_bytes());
                    bytes.extend_from_slice(source);
                    bytes.extend_from_slice(&bits.to_be_bytes());
                    bytes.push(ieee::flags_byte(*flags));
                }
                NumericComputationOperation::Parameter(slot) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&slot.to_be_bytes());
                }
                NumericComputationOperation::Apply {
                    operation,
                    operands,
                } => {
                    bytes.push(2);
                    bytes.push(*operation as u8);
                    bytes.push(operands.len() as u8);
                    for operand in operands {
                        bytes.extend_from_slice(&operand.to_be_bytes());
                    }
                }
            }
            if bytes.len() > MAX_NUMERIC_COMPUTATION_BYTES {
                return Err(NumericComputationRefusal::StorageBudget);
            }
        }
        Ok(bytes)
    }
    pub fn identity(&self) -> Result<[u8; 32], NumericComputationRefusal> {
        Ok(semantic_digest(
            NUMERIC_COMPUTATION_INFO_ID,
            &self.canonical_bytes()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};
    fn literal(value: i64) -> NumericComputationNode {
        NumericComputationNode::literal(
            crate::MATH_VALUE_INFO_ID.to_string(),
            None,
            &MathValue::integer(value),
        )
        .unwrap()
    }
    fn apply(operation: NumericOperation, operands: Vec<u16>) -> NumericComputationNode {
        NumericComputationNode {
            semantic_kind: crate::MATH_VALUE_INFO_ID.to_string(),
            unit: None,
            reference_unit: None,
            operation: NumericComputationOperation::Apply {
                operation,
                operands,
            },
        }
    }
    #[test]
    fn graph_identity_preserves_operand_order_and_intermediate_rounding_sites() {
        let left = NumericComputation::new(
            vec![
                literal(1),
                literal(2),
                apply(NumericOperation::Add, vec![0, 1]),
            ],
            2,
        )
        .unwrap();
        let right = NumericComputation::new(
            vec![
                literal(1),
                literal(2),
                apply(NumericOperation::Add, vec![1, 0]),
            ],
            2,
        )
        .unwrap();
        let folded = NumericComputation::new(vec![literal(3)], 0).unwrap();
        assert_ne!(left.identity().unwrap(), right.identity().unwrap());
        assert_ne!(left.identity().unwrap(), folded.identity().unwrap());
        let wire = left.canonical_bytes().unwrap();
        assert_eq!(NumericComputation::decode_validate(&wire).unwrap(), left);
        let configuration = left.configuration([7; 32]).unwrap();
        assert_eq!(
            NumericComputation::from_configuration(&configuration, [7; 32]).unwrap(),
            left
        );
        assert!(NumericComputation::from_configuration(&configuration, [8; 32]).is_err());
        let mut corrupted = wire;
        corrupted.push(0);
        assert_eq!(
            NumericComputation::decode_validate(&corrupted),
            Err(NumericComputationRefusal::MalformedEncoding)
        );
    }
    #[test]
    fn graph_refuses_cycles_unused_nodes_and_wrong_arities() {
        assert_eq!(
            NumericComputation::new(vec![apply(NumericOperation::Negate, vec![0])], 0),
            Err(NumericComputationRefusal::InvalidOperand)
        );
        assert_eq!(
            NumericComputation::new(vec![literal(1), literal(2)], 0),
            Err(NumericComputationRefusal::UnreachableNode)
        );
        assert_eq!(
            NumericComputation::new(vec![literal(1), apply(NumericOperation::Add, vec![0])], 1),
            Err(NumericComputationRefusal::WrongArity)
        );
    }
}

/// Canonical Scalar profiles preserve the same mathematical ontology.
pub fn is_scalar_kind(kind: &str) -> bool {
    crate::primitive_info_kind(kind) == Some(crate::PrimitiveInfoKind::MathematicalNumber)
}

#[cfg(test)]
mod ieee_tests;
