//! Target-neutral, fully typed program retained from one checked expression.

use crate::{
    BinaryOperator, CheckedExpression, ExpressionProjection, ExpressionSyntax, Span, UnaryOperator,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{StructuredInfoRefusal, StructuredInfoType};

mod checked_encoding;
mod constant;
pub(crate) use checked_encoding::checked_canonical_hex;

pub const MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES: usize = crate::MAXIMUM_PLOT_SOURCE_BYTES * 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableExpressionProgram {
    pub input_type: StructuredInfoType,
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
            input_type: expression
                .input_type
                .structured_info_type_with(&expression.semantic_structures)?,
            output_type: expression
                .value_type
                .structured_info_type_with(&expression.semantic_structures)?,
            root: node(&expression.syntax, expression)?,
        };
        if value.canonical_size()? > MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES {
            return Err(PortableExpressionProgramRefusal::TooLarge);
        }
        Ok(value)
    }

    fn encode(&self, encoded: &mut ProgramSink) -> Result<(), PortableExpressionProgramRefusal> {
        encoded.extend_from_slice(b"conduit.pure-expression.program.v2");
        push_type(encoded, &self.input_type)?;
        push_type(encoded, &self.output_type)?;
        push_node(encoded, &self.root)?;
        if encoded.length > MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES {
            Err(PortableExpressionProgramRefusal::TooLarge)
        } else {
            Ok(())
        }
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

fn node(
    syntax: &ExpressionSyntax,
    checked: &CheckedExpression,
) -> Result<PortableExpressionNode, PortableExpressionProgramRefusal> {
    let value_type = checked
        .node_types
        .iter()
        .find(|node| same_span(node.span, syntax.span()))
        .ok_or(PortableExpressionProgramRefusal::MissingCheckedNodeType)?
        .value_type
        .structured_info_type_with(&checked.semantic_structures)?;
    let operation = match syntax {
        ExpressionSyntax::TypedGlyphLiteral(literal) => {
            let value = checked
                .glyph_values
                .resolve(literal)
                .and_then(crate::CanonicalStructuredStartupValue::try_concrete)
                .ok_or(PortableExpressionProgramRefusal::MissingCheckedNodeType)?;
            if value.value_type() != &value_type {
                return Err(PortableExpressionProgramRefusal::MalformedEncoding);
            }
            PortableExpressionOperation::Constant(value)
        }
        ExpressionSyntax::Input(_) => PortableExpressionOperation::Input,
        ExpressionSyntax::Atomic(value) => {
            if let Some(constant) = checked.immutable_constants.get(&value.text) {
                if constant.value_type() != &value_type {
                    return Err(PortableExpressionProgramRefusal::MalformedEncoding);
                }
                PortableExpressionOperation::Constant(constant.clone())
            } else if let Some(bytes) = checked
                .canonical_literals
                .get(&(value.span.start, value.span.end))
            {
                PortableExpressionOperation::CanonicalLiteral(bytes.clone())
            } else {
                PortableExpressionOperation::Literal(value.text.clone())
            }
        }
        ExpressionSyntax::Projection { value, member, .. }
            if matches!(
                value_type.shape(),
                conduit_core::StructuredInfoTypeShape::Variant { .. }
            ) && matches!(value.as_ref(), ExpressionSyntax::Atomic(_))
                && !checked
                    .node_types
                    .iter()
                    .any(|node| same_span(node.span, value.span())) =>
        {
            let ExpressionProjection::Field(case) = member else {
                return Err(PortableExpressionProgramRefusal::InvalidTupleIndex);
            };
            PortableExpressionOperation::Literal(case.text.clone())
        }
        ExpressionSyntax::Projection { value, member, .. } => {
            PortableExpressionOperation::Projection {
                value: Box::new(node(value, checked)?),
                member: match member {
                    ExpressionProjection::Field(field) => {
                        PortableExpressionProjection::Field(field.text.clone())
                    }
                    ExpressionProjection::TupleIndex(index) => {
                        PortableExpressionProjection::TupleIndex(
                            index
                                .text
                                .parse()
                                .map_err(|_| PortableExpressionProgramRefusal::InvalidTupleIndex)?,
                        )
                    }
                },
            }
        }
        ExpressionSyntax::Unary {
            operator, operand, ..
        } => PortableExpressionOperation::Unary {
            operator: *operator,
            operand: Box::new(node(operand, checked)?),
        },
        ExpressionSyntax::Binary {
            operator,
            left,
            right,
            ..
        } => PortableExpressionOperation::Binary {
            operator: *operator,
            proven: checked
                .proven_arithmetic
                .contains(&(syntax.span().start, syntax.span().end)),
            left: Box::new(node(left, checked)?),
            right: Box::new(node(right, checked)?),
        },
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => PortableExpressionOperation::Conditional {
            condition: Box::new(node(condition, checked)?),
            when_true: Box::new(node(when_true, checked)?),
            when_false: Box::new(node(when_false, checked)?),
        },
        ExpressionSyntax::Tuple { values, .. } => PortableExpressionOperation::Tuple(
            values
                .iter()
                .map(|value| node(value, checked))
                .collect::<Result<_, _>>()?,
        ),
        ExpressionSyntax::Record { fields, .. } => PortableExpressionOperation::Record(
            fields
                .iter()
                .map(|field| Ok((field.name.text.clone(), node(&field.value, checked)?)))
                .collect::<Result<_, PortableExpressionProgramRefusal>>()?,
        ),
        ExpressionSyntax::Collection { values, .. } => PortableExpressionOperation::Collection(
            values
                .iter()
                .map(|value| node(value, checked))
                .collect::<Result<_, _>>()?,
        ),
        ExpressionSyntax::Variant { tag, payload, .. } => PortableExpressionOperation::Variant {
            tag: tag.text.rsplit('.').next().expect("checked tag").into(),
            payload: Box::new(node(payload, checked)?),
        },
        ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } => PortableExpressionOperation::SemanticCall {
            kind: kind.text.clone(),
            arguments: arguments
                .iter()
                .map(|argument| node(argument, checked))
                .collect::<Result<_, _>>()?,
        },
    };
    Ok(PortableExpressionNode {
        value_type,
        operation,
    })
}

fn same_span(left: Span, right: Span) -> bool {
    left.start == right.start && left.end == right.end
}

fn push_node(
    encoded: &mut ProgramSink,
    node: &PortableExpressionNode,
) -> Result<(), PortableExpressionProgramRefusal> {
    push_type(encoded, &node.value_type)?;
    match &node.operation {
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
