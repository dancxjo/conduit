//! Target-neutral, fully typed program retained from one checked expression.

use crate::{
    BinaryOperator, CheckedExpression, ExpressionProjection, ExpressionSyntax, Span, UnaryOperator,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{StructuredInfoRefusal, StructuredInfoType};

mod checked_encoding;
mod checked_node;
use checked_node::{node, same_span};
mod constant;
pub(crate) mod input_bound;
mod qualified_quantity;
pub(crate) mod value_contract;
pub(crate) use checked_encoding::checked_canonical_hex;

pub const MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES: usize = crate::MAXIMUM_PLOT_SOURCE_BYTES * 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableExpressionProgram {
    pub input_type: StructuredInfoType,
    pub input_maximum_bytes: Option<u32>,
    pub output_type: StructuredInfoType,
    pub root: PortableExpressionNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableExpressionNode {
    pub value_type: StructuredInfoType,
    pub operation: PortableExpressionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableExpressionOperation {
    /// Physical body behind qualified evidence ports; requires a certified host profile.
    QualifiedQuantity {
        input: conduit_core::CheckedValueContract,
        output: conduit_core::CheckedValueContract,
        physical_input: StructuredInfoType,
        value: Box<PortableExpressionNode>,
    },
    /// Retain the consuming and emitted predicates at this checked boundary.
    ValueContract {
        input: Option<conduit_core::CheckedValueContract>,
        output: Option<conduit_core::CheckedValueContract>,
        value: Box<PortableExpressionNode>,
    },
    Input,
    Literal(String),
    /// Checked self-contained physical capsule; never reparsed using ambient names.
    CanonicalLiteral(Vec<u8>),
    /// Already admitted ordinary constructor result; no runtime parsing.
    Constant(conduit_core::StructuredInfoValue),
    Projection {
        value: Box<PortableExpressionNode>,
        member: PortableExpressionProjection,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<PortableExpressionNode>,
    },
    Binary {
        operator: BinaryOperator,
        proven: bool,
        left: Box<PortableExpressionNode>,
        right: Box<PortableExpressionNode>,
    },
    Conditional {
        condition: Box<PortableExpressionNode>,
        when_true: Box<PortableExpressionNode>,
        when_false: Box<PortableExpressionNode>,
    },
    Tuple(Vec<PortableExpressionNode>),
    Record(Vec<(String, PortableExpressionNode)>),
    Collection(Vec<PortableExpressionNode>),
    Variant {
        tag: String,
        payload: Box<PortableExpressionNode>,
    },
    SemanticCall {
        kind: String,
        arguments: Vec<PortableExpressionNode>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableExpressionProjection {
    Field(String),
    TupleIndex(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableExpressionProgramRefusal {
    MissingCheckedNodeType,
    InvalidTupleIndex,
    InvalidType(StructuredInfoRefusal),
    MalformedEncoding,
    TooLarge,
}

impl From<StructuredInfoRefusal> for PortableExpressionProgramRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::InvalidType(value)
    }
}

impl PortableExpressionProgram {
    pub fn from_checked(
        expression: &CheckedExpression,
    ) -> Result<Self, PortableExpressionProgramRefusal> {
        let value = Self {
            input_maximum_bytes: expression.input_maximum_bytes,
            input_type: expression
                .input_type
                .structured_info_type_with(&expression.semantic_structures)?,
            output_type: expression
                .value_type
                .structured_info_type_with(&expression.semantic_structures)?,
            root: value_contract::checked_node(expression, node(&expression.syntax, expression)?),
        };
        if value.canonical_size()? > MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES {
            return Err(PortableExpressionProgramRefusal::TooLarge);
        }
        Ok(value)
    }

    fn canonical_size(&self) -> Result<usize, PortableExpressionProgramRefusal> {
        let mut encoded = ProgramSink {
            storage: ProgramStorage::Count,
            length: 0,
        };
        self.encode(&mut encoded)?;
        Ok(encoded.length)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PortableExpressionProgramRefusal> {
        let length = self.canonical_size()?;
        let mut encoded = ProgramSink {
            storage: ProgramStorage::Bytes(Vec::with_capacity(length)),
            length: 0,
        };
        self.encode(&mut encoded)?;
        let ProgramStorage::Bytes(bytes) = encoded.storage else {
            unreachable!("byte encoder owns its admitted storage");
        };
        Ok(bytes)
    }

    pub fn canonical_hex(&self) -> Result<String, PortableExpressionProgramRefusal> {
        let length = self.canonical_size()?;
        let mut encoded = ProgramSink {
            storage: ProgramStorage::Hex(String::with_capacity(length * 2)),
            length: 0,
        };
        self.encode(&mut encoded)?;
        let ProgramStorage::Hex(text) = encoded.storage else {
            unreachable!("hex encoder owns its admitted storage");
        };
        Ok(text)
    }
}

// Counting and writing share the exact same serialization walk. Preparation
// admits the complete byte buffer once, without allocating a disposable copy
// or growing it while the typed program is also retained.
enum ProgramStorage {
    Count,
    Bytes(Vec<u8>),
    Hex(String),
}
struct ProgramSink {
    storage: ProgramStorage,
    length: usize,
}
impl ProgramSink {
    fn push(&mut self, byte: u8) {
        self.extend_from_slice(core::slice::from_ref(&byte));
    }
    fn extend_from_slice(&mut self, value: &[u8]) {
        self.length = self.length.saturating_add(value.len());
        match &mut self.storage {
            ProgramStorage::Count => {}
            ProgramStorage::Bytes(bytes) => bytes.extend_from_slice(value),
            ProgramStorage::Hex(text) => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                for byte in value {
                    text.push(char::from(HEX[usize::from(byte >> 4)]));
                    text.push(char::from(HEX[usize::from(byte & 0x0f)]));
                }
            }
        }
    }
}

fn push_node(
    encoded: &mut ProgramSink,
    node: &PortableExpressionNode,
) -> Result<(), PortableExpressionProgramRefusal> {
    push_type(encoded, &node.value_type)?;
    match &node.operation {
        PortableExpressionOperation::QualifiedQuantity {
            input,
            output,
            physical_input,
            value,
        } => {
            encoded.push(14);
            value_contract::push_contract(encoded, Some(input));
            value_contract::push_contract(encoded, Some(output));
            push_type(encoded, physical_input)?;
            push_node(encoded, value)?;
        }
        PortableExpressionOperation::ValueContract {
            input,
            output,
            value,
        } => {
            encoded.push(13);
            value_contract::push_contract(encoded, input.as_ref());
            value_contract::push_contract(encoded, output.as_ref());
            push_node(encoded, value)?;
        }
        PortableExpressionOperation::Constant(value) => {
            if value.value_type() != &node.value_type {
                return Err(PortableExpressionProgramRefusal::MalformedEncoding);
            }
            let bytes = value.canonical_bytes()?;
            encoded.push(11);
            push_len(encoded, bytes.len());
            encoded.extend_from_slice(&bytes);
        }
        PortableExpressionOperation::Input => encoded.push(0),
        PortableExpressionOperation::Literal(value) => {
            encoded.push(1);
            push_text(encoded, value);
        }
        PortableExpressionOperation::CanonicalLiteral(value) => {
            validate_capsule_literal(&node.value_type, value)?;
            encoded.push(12);
            push_len(encoded, value.len());
            encoded.extend_from_slice(value);
        }
        PortableExpressionOperation::Projection { value, member } => {
            encoded.push(2);
            push_node(encoded, value)?;
            match member {
                PortableExpressionProjection::Field(field) => {
                    encoded.push(0);
                    push_text(encoded, field);
                }
                PortableExpressionProjection::TupleIndex(index) => {
                    encoded.push(1);
                    encoded.extend_from_slice(&index.to_le_bytes());
                }
            }
        }
        PortableExpressionOperation::Unary { operator, operand } => {
            encoded.push(3);
            encoded.push(unary_tag(*operator));
            push_node(encoded, operand)?;
        }
        PortableExpressionOperation::Binary {
            operator,
            proven,
            left,
            right,
        } => {
            encoded.push(4);
            encoded.push(binary_tag(*operator));
            encoded.push(u8::from(*proven));
            push_node(encoded, left)?;
            push_node(encoded, right)?;
        }
        PortableExpressionOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            encoded.push(5);
            push_node(encoded, condition)?;
            push_node(encoded, when_true)?;
            push_node(encoded, when_false)?;
        }
        PortableExpressionOperation::Tuple(values) => {
            encoded.push(6);
            push_nodes(encoded, values)?;
        }
        PortableExpressionOperation::Record(fields) => {
            encoded.push(7);
            push_len(encoded, fields.len());
            for (name, value) in fields {
                push_text(encoded, name);
                push_node(encoded, value)?;
            }
        }
        PortableExpressionOperation::Collection(values) => {
            encoded.push(8);
            push_nodes(encoded, values)?;
        }
        PortableExpressionOperation::Variant { tag, payload } => {
            encoded.push(9);
            push_text(encoded, tag);
            push_node(encoded, payload)?;
        }
        PortableExpressionOperation::SemanticCall { kind, arguments } => {
            encoded.push(10);
            push_text(encoded, kind);
            push_nodes(encoded, arguments)?;
        }
    }
    Ok(())
}

fn push_nodes(
    encoded: &mut ProgramSink,
    values: &[PortableExpressionNode],
) -> Result<(), PortableExpressionProgramRefusal> {
    push_len(encoded, values.len());
    for value in values {
        push_node(encoded, value)?;
    }
    Ok(())
}

fn push_type(
    encoded: &mut ProgramSink,
    value_type: &StructuredInfoType,
) -> Result<(), PortableExpressionProgramRefusal> {
    let bytes = value_type.canonical_bytes()?;
    push_len(encoded, bytes.len());
    encoded.extend_from_slice(&bytes);
    Ok(())
}

fn push_text(encoded: &mut ProgramSink, value: &str) {
    push_len(encoded, value.len());
    encoded.extend_from_slice(value.as_bytes());
}

fn push_len(encoded: &mut ProgramSink, value: usize) {
    encoded.extend_from_slice(&(value as u64).to_le_bytes());
}

const fn unary_tag(operator: UnaryOperator) -> u8 {
    match operator {
        UnaryOperator::Not => 0,
        UnaryOperator::Negate => 1,
    }
}

const fn binary_tag(operator: BinaryOperator) -> u8 {
    match operator {
        BinaryOperator::Multiply => 0,
        BinaryOperator::Divide => 1,
        BinaryOperator::Remainder => 2,
        BinaryOperator::Add => 3,
        BinaryOperator::Subtract => 4,
        BinaryOperator::ShiftLeft => 5,
        BinaryOperator::ShiftRight => 6,
        BinaryOperator::Less => 7,
        BinaryOperator::LessOrEqual => 8,
        BinaryOperator::Greater => 9,
        BinaryOperator::GreaterOrEqual => 10,
        BinaryOperator::Equal => 11,
        BinaryOperator::NotEqual => 12,
        BinaryOperator::BitAnd => 13,
        BinaryOperator::BitXor => 14,
        BinaryOperator::BitOr => 15,
        BinaryOperator::BooleanAnd => 16,
        BinaryOperator::BooleanOr => 17,
    }
}

pub(crate) fn validate_capsule_literal(
    ty: &StructuredInfoType,
    bytes: &[u8],
) -> Result<(), PortableExpressionProgramRefusal> {
    let conduit_core::StructuredInfoTypeShape::Leaf(kind) = ty.shape() else {
        return Err(PortableExpressionProgramRefusal::MalformedEncoding);
    };
    if crate::authored_quantity::expected_role(ty).is_none()
        || conduit_core::validate_primitive_info(kind.as_str(), bytes).is_err()
    {
        return Err(PortableExpressionProgramRefusal::MalformedEncoding);
    }
    Ok(())
}
