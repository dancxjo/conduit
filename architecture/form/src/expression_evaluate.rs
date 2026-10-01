//! Portable semantic evaluation for one already checked expression program.

mod structured;
use structured::{encoded_structured, projection, structured_record, structured_value};

use crate::{
    BinaryOperator, PortableExpressionNode, PortableExpressionOperation, PortableExpressionProgram,
    PortableExpressionProjection, UnaryOperator,
};
use alloc::{string::String, vec::Vec};
use conduit_core::{
    decode_count, encode_count, primitive_info_kind, FixedInteger, InfoBool, PrimitiveInfoKind,
    Quantity, Scalar, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape, BOOL_INFO_ID, COUNT_INFO_ID, SCALAR_INFO_ID,
    TEXT_INFO_ID,
};
use core::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableExpressionEvaluationRefusal {
    InvalidInput,
    InvalidProgram,
    InvalidLiteral,
    Arithmetic,
    UnsupportedSemanticCall(String),
    UnsupportedType(String),
}

impl PortableExpressionProgram {
    /// Evaluates checked meaning without host-sized arithmetic, implicit
    /// promotion or eager evaluation of an unselected conditional branch.
    ///
    /// This allocating conformance entrance is not itself a Play Back. Host
    /// installations prepare bounded scratch storage before exposing the same
    /// evaluator through the execution kernel.
    pub fn evaluate(&self, input: &[u8]) -> Result<Vec<u8>, PortableExpressionEvaluationRefusal> {
        validate_input(&self.input_type, input)?;
        let output = evaluate_node(&self.root, input, &self.input_type)?;
        if output.value_type != self.output_type {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        Ok(output.encoded)
    }
}

pub(super) struct Value {
    pub(super) value_type: StructuredInfoType,
    pub(super) encoded: Vec<u8>,
}

fn evaluate_node(
    node: &PortableExpressionNode,
    input: &[u8],
    input_type: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    let value = match &node.operation {
        PortableExpressionOperation::Input => {
            if &node.value_type != input_type {
                return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
            }
            Value {
                value_type: node.value_type.clone(),
                encoded: input.to_vec(),
            }
        }
        PortableExpressionOperation::Literal(literal) => literal_value(&node.value_type, literal)?,
        PortableExpressionOperation::Projection { value, member } => projection(
            evaluate_node(value, input, input_type)?,
            member,
            &node.value_type,
        )?,
        PortableExpressionOperation::Unary { operator, operand } => unary(
            *operator,
            evaluate_node(operand, input, input_type)?,
            &node.value_type,
        )?,
        PortableExpressionOperation::Binary {
            operator,
            proven,
            left,
            right,
        } => binary(
            *operator,
            *proven,
            evaluate_node(left, input, input_type)?,
            evaluate_node(right, input, input_type)?,
            &node.value_type,
        )?,
        PortableExpressionOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            let condition = evaluate_node(condition, input, input_type)?;
            let selected = if decode_bool(&condition)? {
                when_true
            } else {
                when_false
            };
            evaluate_node(selected, input, input_type)?
        }
        PortableExpressionOperation::Tuple(values) => {
            let fields = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    StructuredFieldValue::new(
                        format!("item-{index:05}"),
                        structured_value(evaluate_node(value, input, input_type)?)?,
                    )
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)
                })
                .collect::<Result<Vec<_>, PortableExpressionEvaluationRefusal>>()?;
            structured_record(node.value_type.clone(), fields)?
        }
        PortableExpressionOperation::Record(fields) => {
            let fields = fields
                .iter()
                .map(|(name, value)| {
                    StructuredFieldValue::new(
                        name.clone(),
                        structured_value(evaluate_node(value, input, input_type)?)?,
                    )
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)
                })
                .collect::<Result<Vec<_>, PortableExpressionEvaluationRefusal>>()?;
            structured_record(node.value_type.clone(), fields)?
        }
        PortableExpressionOperation::Collection(values) => {
            let values = values
                .iter()
                .map(|value| structured_value(evaluate_node(value, input, input_type)?))
                .collect::<Result<Vec<_>, _>>()?;
            let value = StructuredInfoValue::collection(node.value_type.clone(), values)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
            encoded_structured(value)?
        }
        PortableExpressionOperation::Variant { .. } => {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram)
        }
        PortableExpressionOperation::SemanticCall { kind, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| evaluate_node(argument, input, input_type))
                .collect::<Result<Vec<_>, _>>()?;
            intrinsic_call(kind, arguments, &node.value_type)?
        }
    };
    if value.value_type != node.value_type {
        Err(PortableExpressionEvaluationRefusal::InvalidProgram)
    } else {
        Ok(value)
    }
}

fn intrinsic_call(
    kind: &str,
    mut arguments: Vec<Value>,
    expected: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    if kind == "variant/tag" {
        let argument = arguments
            .pop()
            .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
        if !arguments.is_empty() {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        let value = StructuredInfoValue::from_canonical_bytes(&argument.encoded)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        };
        return Ok(Value {
            value_type: expected.clone(),
            encoded: tag.as_bytes().to_vec(),
        });
    }
    if matches!(
        kind,
        "value/u16"
            | "value/u32"
            | "value/u64"
            | "value/u128"
            | "value/i16"
            | "value/i32"
            | "value/i64"
            | "value/i128"
    ) {
        let argument = arguments
            .pop()
            .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
        if !arguments.is_empty() {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        let source = decode_integer(&argument)?;
        let target = primitive_info_kind(leaf_kind(expected)?)
            .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let widened = if signed_integer(source.kind()) {
            FixedInteger::from_signed(
                target,
                source
                    .signed()
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
            )
        } else if signed_integer(target) {
            FixedInteger::from_signed(
                target,
                i128::try_from(
                    source
                        .unsigned()
                        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
                )
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
            )
        } else {
            FixedInteger::from_unsigned(
                target,
                source
                    .unsigned()
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
            )
        }
        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        return Ok(Value {
            value_type: expected.clone(),
            encoded: encode_integer(widened),
        });
    }
    Err(PortableExpressionEvaluationRefusal::UnsupportedSemanticCall(kind.into()))
}

fn validate_input(
    value_type: &StructuredInfoType,
    encoded: &[u8],
) -> Result<(), PortableExpressionEvaluationRefusal> {
    match value_type.shape() {
        StructuredInfoTypeShape::Leaf(kind) => {
            conduit_core::validate_primitive_info(kind.as_str(), encoded)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)
        }
        _ => {
            let value = StructuredInfoValue::from_canonical_bytes(encoded)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)?;
            if value.value_type() == value_type {
                Ok(())
            } else {
                Err(PortableExpressionEvaluationRefusal::InvalidInput)
            }
        }
    }
}

fn literal_value(
    value_type: &StructuredInfoType,
    literal: &str,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    let kind = leaf_kind(value_type)?;
    let encoded = match primitive_info_kind(kind) {
        Some(PrimitiveInfoKind::Bool) => match literal {
            "true" => InfoBool::TRUE.encode().to_vec(),
            "false" => InfoBool::FALSE.encode().to_vec(),
            _ => return Err(PortableExpressionEvaluationRefusal::InvalidLiteral),
        },
        Some(PrimitiveInfoKind::Text) => crate::text_value::parse_quoted_text(literal)
            .ok_or(PortableExpressionEvaluationRefusal::InvalidLiteral)?
            .into_bytes(),
        Some(PrimitiveInfoKind::Count) => encode_count(
            literal
                .parse()
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?,
        )
        .to_vec(),
        Some(PrimitiveInfoKind::Scalar) => crate::structured_startup::parse_scalar_literal(literal)
            .ok_or(PortableExpressionEvaluationRefusal::InvalidLiteral)?
            .encode()
            .to_vec(),
        Some(kind) if quantity_kind(kind) => {
            let quantity = Quantity::parse_form_literal(literal)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?;
            conduit_core::validate_primitive_info(leaf_kind(value_type)?, &quantity.encode())
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?;
            quantity.encode().to_vec()
        }
        Some(kind) if fixed_integer(kind) => {
            let canonical = crate::integer_literal::canonicalize(literal, leaf_kind(value_type)?)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?
                .ok_or(PortableExpressionEvaluationRefusal::InvalidLiteral)?;
            let integer = if signed_integer(kind) {
                FixedInteger::from_signed(
                    kind,
                    canonical
                        .parse()
                        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?,
                )
            } else {
                FixedInteger::from_unsigned(
                    kind,
                    canonical
                        .parse()
                        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?,
                )
            }
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?;
            encode_integer(integer)
        }
        _ => {
            return Err(PortableExpressionEvaluationRefusal::UnsupportedType(
                kind.into(),
            ))
        }
    };
    primitive_value(value_type, encoded)
}

pub(super) fn literal_primitive_bytes(
    value_type: &StructuredInfoType,
    literal: &str,
) -> Result<Vec<u8>, PortableExpressionEvaluationRefusal> {
    primitive_bytes(&literal_value(value_type, literal)?)
}

fn unary(
    operator: UnaryOperator,
    operand: Value,
    expected: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    let encoded = match operator {
        UnaryOperator::Not => InfoBool::new(!decode_bool(&operand)?).encode().to_vec(),
        UnaryOperator::Negate => {
            if leaf_kind(&operand.value_type)? == SCALAR_INFO_ID {
                let value = Scalar::decode(&primitive_bytes(&operand)?)
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
                value
                    .checked_neg()
                    .map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)?
                    .encode()
                    .to_vec()
            } else {
                encode_integer(
                    decode_integer(&operand)?
                        .checked_neg()
                        .map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)?,
                )
            }
        }
    };
    primitive_value(expected, encoded)
}

fn binary(
    operator: BinaryOperator,
    proven: bool,
    left: Value,
    right: Value,
    expected: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    if left.value_type != right.value_type {
        return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
    }
    let comparison = matches!(
        operator,
        BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Equal
            | BinaryOperator::NotEqual
    );
    let encoded = if matches!(
        operator,
        BinaryOperator::BooleanAnd | BinaryOperator::BooleanOr
    ) {
        let left = decode_bool(&left)?;
        let right = decode_bool(&right)?;
        InfoBool::new(match operator {
            BinaryOperator::BooleanAnd => left && right,
            BinaryOperator::BooleanOr => left || right,
            _ => unreachable!(),
        })
        .encode()
        .to_vec()
    } else if comparison {
        let ordering = compare(&left, &right)?;
        InfoBool::new(match operator {
            BinaryOperator::Less => ordering.is_lt(),
            BinaryOperator::LessOrEqual => !ordering.is_gt(),
            BinaryOperator::Greater => ordering.is_gt(),
            BinaryOperator::GreaterOrEqual => !ordering.is_lt(),
            BinaryOperator::Equal => ordering.is_eq(),
            BinaryOperator::NotEqual => !ordering.is_eq(),
            _ => unreachable!(),
        })
        .encode()
        .to_vec()
    } else if leaf_kind(&left.value_type)? == COUNT_INFO_ID {
        let left = decode_count(&primitive_bytes(&left)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let right = decode_count(&primitive_bytes(&right)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let result = match operator {
            BinaryOperator::Multiply => left.checked_mul(right),
            BinaryOperator::Divide if right != 0 => left.checked_div(right),
            BinaryOperator::Remainder if right != 0 => left.checked_rem(right),
            BinaryOperator::Add => left.checked_add(right),
            BinaryOperator::Subtract => left.checked_sub(right),
            _ => None,
        }
        .ok_or(PortableExpressionEvaluationRefusal::Arithmetic)?;
        encode_count(result).to_vec()
    } else if leaf_kind(&left.value_type)? == SCALAR_INFO_ID {
        let left = Scalar::decode(&primitive_bytes(&left)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let right = Scalar::decode(&primitive_bytes(&right)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        match operator {
            BinaryOperator::Multiply => left.checked_mul(right),
            BinaryOperator::Divide => left.checked_div(right),
            BinaryOperator::Remainder => left.checked_rem(right),
            BinaryOperator::Add => left.checked_add(right),
            BinaryOperator::Subtract => left.checked_sub(right),
            _ => return Err(PortableExpressionEvaluationRefusal::InvalidProgram),
        }
        .map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)?
        .encode()
        .to_vec()
    } else {
        let left = decode_integer(&left)?;
        let right_integer = decode_integer(&right)?;
        let result = match (operator, proven) {
            (BinaryOperator::Multiply, true) => left.wrapping_mul(right_integer),
            (BinaryOperator::Add, true) => left.wrapping_add(right_integer),
            (BinaryOperator::Subtract, true) => left.wrapping_sub(right_integer),
            (BinaryOperator::Multiply, false) => left.checked_mul(right_integer),
            (BinaryOperator::Divide, _) => left.checked_div(right_integer),
            (BinaryOperator::Remainder, _) => left.checked_rem(right_integer),
            (BinaryOperator::Add, false) => left.checked_add(right_integer),
            (BinaryOperator::Subtract, false) => left.checked_sub(right_integer),
            (BinaryOperator::BitAnd, _) => left.bit_and(right_integer),
            (BinaryOperator::BitXor, _) => left.bit_xor(right_integer),
            (BinaryOperator::BitOr, _) => left.bit_or(right_integer),
            (BinaryOperator::ShiftLeft, _) => left.checked_shift_left(shift_count(right_integer)?),
            (BinaryOperator::ShiftRight, _) => {
                left.checked_shift_right(shift_count(right_integer)?)
            }
            _ => return Err(PortableExpressionEvaluationRefusal::InvalidProgram),
        }
        .map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)?;
        encode_integer(result)
    };
    primitive_value(expected, encoded)
}

fn compare(left: &Value, right: &Value) -> Result<Ordering, PortableExpressionEvaluationRefusal> {
    let kind = leaf_kind(&left.value_type)?;
    if fixed_integer(
        primitive_info_kind(kind)
            .ok_or_else(|| PortableExpressionEvaluationRefusal::UnsupportedType(kind.into()))?,
    ) {
        decode_integer(left)?
            .compare(decode_integer(right)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)
    } else if kind == BOOL_INFO_ID || kind == TEXT_INFO_ID {
        Ok(left.encoded.cmp(&right.encoded))
    } else if kind == COUNT_INFO_ID {
        Ok(decode_count(&primitive_bytes(left)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?
            .cmp(
                &decode_count(&primitive_bytes(right)?)
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
            ))
    } else if kind == SCALAR_INFO_ID {
        Ok(Scalar::decode(&primitive_bytes(left)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?
            .cmp(
                &Scalar::decode(&primitive_bytes(right)?)
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
            ))
    } else if conduit_core::quantity_info_dimension(kind).is_some()
        || kind == conduit_core::QUANTITY_INFO_ID
    {
        Quantity::decode(&left.encoded)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?
            .compare(
                Quantity::decode(&right.encoded)
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
            )
            .map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)
    } else {
        Err(PortableExpressionEvaluationRefusal::UnsupportedType(
            kind.into(),
        ))
    }
}

fn decode_bool(value: &Value) -> Result<bool, PortableExpressionEvaluationRefusal> {
    if leaf_kind(&value.value_type)? != BOOL_INFO_ID {
        return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
    }
    InfoBool::decode(&primitive_bytes(value)?)
        .map(InfoBool::get)
        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)
}

fn decode_integer(value: &Value) -> Result<FixedInteger, PortableExpressionEvaluationRefusal> {
    let kind = primitive_info_kind(leaf_kind(&value.value_type)?)
        .filter(|kind| fixed_integer(*kind))
        .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
    FixedInteger::decode(kind, &primitive_bytes(value)?)
        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)
}

fn primitive_bytes(value: &Value) -> Result<Vec<u8>, PortableExpressionEvaluationRefusal> {
    if matches!(
        value.value_type.shape(),
        StructuredInfoTypeShape::Nominal { .. }
    ) {
        let structured = StructuredInfoValue::from_canonical_bytes(&value.encoded)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let StructuredInfoValueShape::Leaf(bytes) = structured.shape() else {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        };
        return Ok(bytes.to_vec());
    }
    Ok(value.encoded.clone())
}

fn encode_integer(value: FixedInteger) -> Vec<u8> {
    let (encoded, length) = value.encode();
    encoded[..length].to_vec()
}

fn shift_count(value: FixedInteger) -> Result<u32, PortableExpressionEvaluationRefusal> {
    let value = if signed_integer(value.kind()) {
        u128::try_from(
            value
                .signed()
                .map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)?,
        )
        .map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)?
    } else {
        value
            .unsigned()
            .map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)?
    };
    u32::try_from(value).map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)
}

fn leaf_kind(value_type: &StructuredInfoType) -> Result<&str, PortableExpressionEvaluationRefusal> {
    match value_type.shape() {
        StructuredInfoTypeShape::Leaf(kind) => Ok(kind.as_str()),
        StructuredInfoTypeShape::Nominal { representation, .. } => leaf_kind(representation),
        _ => Err(PortableExpressionEvaluationRefusal::InvalidProgram),
    }
}

fn primitive_value(
    value_type: &StructuredInfoType,
    encoded: Vec<u8>,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    if let StructuredInfoTypeShape::Nominal { representation, .. } = value_type.shape() {
        let representation = StructuredInfoValue::leaf(representation.clone(), encoded)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let nominal = StructuredInfoValue::nominal(value_type.clone(), representation)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        return Ok(Value {
            value_type: value_type.clone(),
            encoded: nominal
                .canonical_bytes()
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
        });
    }
    Ok(Value {
        value_type: value_type.clone(),
        encoded,
    })
}

const fn fixed_integer(kind: PrimitiveInfoKind) -> bool {
    matches!(
        kind,
        PrimitiveInfoKind::U8
            | PrimitiveInfoKind::U16
            | PrimitiveInfoKind::U32
            | PrimitiveInfoKind::U64
            | PrimitiveInfoKind::U128
            | PrimitiveInfoKind::I8
            | PrimitiveInfoKind::I16
            | PrimitiveInfoKind::I32
            | PrimitiveInfoKind::I64
            | PrimitiveInfoKind::I128
    )
}

const fn signed_integer(kind: PrimitiveInfoKind) -> bool {
    matches!(
        kind,
        PrimitiveInfoKind::I8
            | PrimitiveInfoKind::I16
            | PrimitiveInfoKind::I32
            | PrimitiveInfoKind::I64
            | PrimitiveInfoKind::I128
    )
}

const fn quantity_kind(kind: PrimitiveInfoKind) -> bool {
    matches!(
        kind,
        PrimitiveInfoKind::Quantity
            | PrimitiveInfoKind::Distance
            | PrimitiveInfoKind::Frequency
            | PrimitiveInfoKind::Duration
            | PrimitiveInfoKind::Voltage
            | PrimitiveInfoKind::Temperature
            | PrimitiveInfoKind::Angle
            | PrimitiveInfoKind::Ratio
            | PrimitiveInfoKind::PixelCount
    )
}
