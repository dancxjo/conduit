//! Strict bounded decoder for the portable expression program contract.

use crate::{
    BinaryOperator, PortableExpressionNode, PortableExpressionOperation, PortableExpressionProgram,
    PortableExpressionProgramRefusal, PortableExpressionProjection, UnaryOperator,
    MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{
    StructuredInfoType, MAXIMUM_STRUCTURED_INFO_DEPTH, MAXIMUM_STRUCTURED_INFO_NODES,
    MAXIMUM_STRUCTURED_NAME_BYTES,
};

const HEADER: &[u8] = b"conduit.pure-expression.program.v2";

impl PortableExpressionProgram {
    pub fn from_canonical_bytes(encoded: &[u8]) -> Result<Self, PortableExpressionProgramRefusal> {
        if encoded.len() > MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES {
            return Err(PortableExpressionProgramRefusal::TooLarge);
        }
        let mut cursor = Cursor { remaining: encoded };
        cursor.expect(HEADER)?;
        let input_type = cursor.value_type()?;
        let output_type = cursor.value_type()?;
        let mut remaining_nodes = MAXIMUM_STRUCTURED_INFO_NODES;
        let root = cursor.node(0, &mut remaining_nodes)?;
        if !cursor.remaining.is_empty() || root.value_type != output_type {
            return Err(PortableExpressionProgramRefusal::MalformedEncoding);
        }
        let value = Self {
            input_type,
            output_type,
            root,
        };
        if value.canonical_bytes()?.as_slice() != encoded {
            return Err(PortableExpressionProgramRefusal::MalformedEncoding);
        }
        Ok(value)
    }

    pub fn from_canonical_hex(encoded: &str) -> Result<Self, PortableExpressionProgramRefusal> {
        if !encoded.len().is_multiple_of(2)
            || encoded.len() > MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES.saturating_mul(2)
        {
            return Err(PortableExpressionProgramRefusal::MalformedEncoding);
        }
        let mut bytes = Vec::with_capacity(encoded.len() / 2);
        for pair in encoded.as_bytes().as_chunks::<2>().0 {
            let high = hex(pair[0])?;
            let low = hex(pair[1])?;
            bytes.push((high << 4) | low);
        }
        Self::from_canonical_bytes(&bytes)
    }
}

struct Cursor<'a> {
    remaining: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn expect(&mut self, expected: &[u8]) -> Result<(), PortableExpressionProgramRefusal> {
        let actual = self.take(expected.len())?;
        if actual == expected {
            Ok(())
        } else {
            Err(PortableExpressionProgramRefusal::MalformedEncoding)
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], PortableExpressionProgramRefusal> {
        let (value, remaining) = self
            .remaining
            .split_at_checked(length)
            .ok_or(PortableExpressionProgramRefusal::MalformedEncoding)?;
        self.remaining = remaining;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, PortableExpressionProgramRefusal> {
        Ok(self.take(1)?[0])
    }

    fn length(&mut self) -> Result<usize, PortableExpressionProgramRefusal> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .map_err(|_| PortableExpressionProgramRefusal::MalformedEncoding)?;
        usize::try_from(u64::from_le_bytes(bytes))
            .map_err(|_| PortableExpressionProgramRefusal::MalformedEncoding)
    }

    fn text(&mut self) -> Result<String, PortableExpressionProgramRefusal> {
        let length = self.length()?;
        if length > MAXIMUM_STRUCTURED_NAME_BYTES.max(crate::MAXIMUM_PLOT_SOURCE_BYTES) {
            return Err(PortableExpressionProgramRefusal::MalformedEncoding);
        }
        core::str::from_utf8(self.take(length)?)
            .map(String::from)
            .map_err(|_| PortableExpressionProgramRefusal::MalformedEncoding)
    }

    fn value_type(&mut self) -> Result<StructuredInfoType, PortableExpressionProgramRefusal> {
        let length = self.length()?;
        let encoded = self.take(length)?;
        StructuredInfoType::from_canonical_bytes(encoded).map_err(Into::into)
    }

    fn node(
        &mut self,
        depth: usize,
        remaining_nodes: &mut usize,
    ) -> Result<PortableExpressionNode, PortableExpressionProgramRefusal> {
        if depth > MAXIMUM_STRUCTURED_INFO_DEPTH {
            return Err(PortableExpressionProgramRefusal::MalformedEncoding);
        }
        *remaining_nodes = remaining_nodes
            .checked_sub(1)
            .ok_or(PortableExpressionProgramRefusal::MalformedEncoding)?;
        let value_type = self.value_type()?;
        let operation = match self.byte()? {
            11 => {
                let length = self.length()?;
                if length > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES {
                    return Err(PortableExpressionProgramRefusal::TooLarge);
                }
                let value =
                    conduit_core::StructuredInfoValue::from_canonical_bytes(self.take(length)?)?;
                if value.value_type() != &value_type {
                    return Err(PortableExpressionProgramRefusal::MalformedEncoding);
                }
                PortableExpressionOperation::Constant(value)
            }
            0 => PortableExpressionOperation::Input,
            1 => PortableExpressionOperation::Literal(self.text()?),
            12 => {
                let length = self.length()?;
                let bytes = self.take(length)?.to_vec();
                crate::expression_program::validate_capsule_literal(&value_type, &bytes)?;
                PortableExpressionOperation::CanonicalLiteral(bytes)
            }
            2 => {
                let value = Box::new(self.node(depth + 1, remaining_nodes)?);
                let member = match self.byte()? {
                    0 => PortableExpressionProjection::Field(self.text()?),
                    1 => PortableExpressionProjection::TupleIndex(u16::from_le_bytes(
                        self.take(2)?
                            .try_into()
                            .map_err(|_| PortableExpressionProgramRefusal::MalformedEncoding)?,
                    )),
                    _ => return Err(PortableExpressionProgramRefusal::MalformedEncoding),
                };
                PortableExpressionOperation::Projection { value, member }
            }
            3 => PortableExpressionOperation::Unary {
                operator: unary(self.byte()?)?,
                operand: Box::new(self.node(depth + 1, remaining_nodes)?),
            },
            4 => PortableExpressionOperation::Binary {
                operator: binary(self.byte()?)?,
                proven: match self.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(PortableExpressionProgramRefusal::MalformedEncoding),
                },
                left: Box::new(self.node(depth + 1, remaining_nodes)?),
                right: Box::new(self.node(depth + 1, remaining_nodes)?),
            },
            5 => PortableExpressionOperation::Conditional {
                condition: Box::new(self.node(depth + 1, remaining_nodes)?),
                when_true: Box::new(self.node(depth + 1, remaining_nodes)?),
                when_false: Box::new(self.node(depth + 1, remaining_nodes)?),
            },
            6 => PortableExpressionOperation::Tuple(self.nodes(depth, remaining_nodes)?),
            7 => {
                let length = self.bounded_node_count(*remaining_nodes)?;
                let mut fields = Vec::with_capacity(length);
                for _ in 0..length {
                    let name = self.text()?;
                    if name.is_empty() || name.len() > MAXIMUM_STRUCTURED_NAME_BYTES {
                        return Err(PortableExpressionProgramRefusal::MalformedEncoding);
                    }
                    fields.push((name, self.node(depth + 1, remaining_nodes)?));
                }
                PortableExpressionOperation::Record(fields)
            }
            8 => PortableExpressionOperation::Collection(self.nodes(depth, remaining_nodes)?),
            9 => PortableExpressionOperation::Variant {
                tag: self.text()?,
                payload: Box::new(self.node(depth + 1, remaining_nodes)?),
            },
            10 => PortableExpressionOperation::SemanticCall {
                kind: self.text()?,
                arguments: self.nodes(depth, remaining_nodes)?,
            },
            _ => return Err(PortableExpressionProgramRefusal::MalformedEncoding),
        };
        Ok(PortableExpressionNode {
            value_type,
            operation,
        })
    }

    fn nodes(
        &mut self,
        depth: usize,
        remaining_nodes: &mut usize,
    ) -> Result<Vec<PortableExpressionNode>, PortableExpressionProgramRefusal> {
        let length = self.bounded_node_count(*remaining_nodes)?;
        (0..length)
            .map(|_| self.node(depth + 1, remaining_nodes))
            .collect()
    }

    fn bounded_node_count(
        &mut self,
        remaining_nodes: usize,
    ) -> Result<usize, PortableExpressionProgramRefusal> {
        let length = self.length()?;
        if length > remaining_nodes {
            Err(PortableExpressionProgramRefusal::MalformedEncoding)
        } else {
            Ok(length)
        }
    }
}

fn hex(value: u8) -> Result<u8, PortableExpressionProgramRefusal> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(PortableExpressionProgramRefusal::MalformedEncoding),
    }
}

fn unary(value: u8) -> Result<UnaryOperator, PortableExpressionProgramRefusal> {
    match value {
        0 => Ok(UnaryOperator::Not),
        1 => Ok(UnaryOperator::Negate),
        _ => Err(PortableExpressionProgramRefusal::MalformedEncoding),
    }
}

fn binary(value: u8) -> Result<BinaryOperator, PortableExpressionProgramRefusal> {
    match value {
        0 => Ok(BinaryOperator::Multiply),
        1 => Ok(BinaryOperator::Divide),
        2 => Ok(BinaryOperator::Remainder),
        3 => Ok(BinaryOperator::Add),
        4 => Ok(BinaryOperator::Subtract),
        5 => Ok(BinaryOperator::ShiftLeft),
        6 => Ok(BinaryOperator::ShiftRight),
        7 => Ok(BinaryOperator::Less),
        8 => Ok(BinaryOperator::LessOrEqual),
        9 => Ok(BinaryOperator::Greater),
        10 => Ok(BinaryOperator::GreaterOrEqual),
        11 => Ok(BinaryOperator::Equal),
        12 => Ok(BinaryOperator::NotEqual),
        13 => Ok(BinaryOperator::BitAnd),
        14 => Ok(BinaryOperator::BitXor),
        15 => Ok(BinaryOperator::BitOr),
        16 => Ok(BinaryOperator::BooleanAnd),
        17 => Ok(BinaryOperator::BooleanOr),
        _ => Err(PortableExpressionProgramRefusal::MalformedEncoding),
    }
}
