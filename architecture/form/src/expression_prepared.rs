//! Pre-Play preparation and allocation-free evaluation for primitive expressions.

use crate::{
    BinaryOperator, PortableExpressionEvaluationRefusal as Refusal, PortableExpressionNode,
    PortableExpressionOperation, PortableExpressionProgram, UnaryOperator,
};
use alloc::{boxed::Box, string::ToString, vec, vec::Vec};
use conduit_core::{
    decode_count, encode_count, primitive_info_kind, FixedInteger, InfoBool, PrimitiveInfoKind,
    Quantity, QuantityUnit, Scalar, StructuredInfoTypeShape, BOOL_INFO_ID, COUNT_INFO_ID,
    MAXIMUM_STRUCTURED_LEAF_BYTES, SCALAR_INFO_ID,
};
use core::cmp::Ordering;

/// A prepared primitive-only evaluator. Construction owns every allocation;
/// `evaluate` uses fixed stack values and one capacity-stable output buffer.
pub struct PreparedPortableExpressionEvaluator {
    root: PreparedNode,
    input_kind: PrimitiveInfoKind,
    output: Vec<u8>,
}

struct PreparedNode {
    kind: PrimitiveInfoKind,
    operation: PreparedOperation,
}

enum PreparedOperation {
    Input,
    Literal(Vec<u8>),
    Unary {
        operator: UnaryOperator,
        operand: Box<PreparedNode>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<PreparedNode>,
        right: Box<PreparedNode>,
    },
    Conditional {
        condition: Box<PreparedNode>,
        when_true: Box<PreparedNode>,
        when_false: Box<PreparedNode>,
    },
}

struct PrimitiveValue {
    kind: PrimitiveInfoKind,
    length: usize,
    bytes: [u8; MAXIMUM_STRUCTURED_LEAF_BYTES],
}

impl PrimitiveValue {
    fn new(kind: PrimitiveInfoKind, encoded: &[u8]) -> Result<Self, Refusal> {
        if encoded.len() > MAXIMUM_STRUCTURED_LEAF_BYTES {
            return Err(Refusal::InvalidInput);
        }
        let mut bytes = [0; MAXIMUM_STRUCTURED_LEAF_BYTES];
        bytes[..encoded.len()].copy_from_slice(encoded);
        Ok(Self {
            kind,
            length: encoded.len(),
            bytes,
        })
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

impl PreparedPortableExpressionEvaluator {
    pub fn new(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        let input_kind = leaf_kind(&program.input_type)?;
        let root = prepare_node(&program.root)?;
        if root.kind != leaf_kind(&program.output_type)? {
            return Err(Refusal::InvalidProgram);
        }
        Ok(Self {
            root,
            input_kind,
            output: vec![0; MAXIMUM_STRUCTURED_LEAF_BYTES],
        })
    }

    pub fn evaluate(&mut self, input: &[u8]) -> Result<&[u8], Refusal> {
        conduit_core::validate_primitive_info(kind_name(self.input_kind), input)
            .map_err(|_| Refusal::InvalidInput)?;
        let value = evaluate_node(&self.root, input, self.input_kind)?;
        self.output[..value.length].copy_from_slice(value.as_slice());
        Ok(&self.output[..value.length])
    }
}

fn prepare_node(node: &PortableExpressionNode) -> Result<PreparedNode, Refusal> {
    let kind = leaf_kind(&node.value_type)?;
    let operation = match &node.operation {
        PortableExpressionOperation::Input => PreparedOperation::Input,
        PortableExpressionOperation::Literal(literal) => {
            let one = PortableExpressionProgram {
                input_type: node.value_type.clone(),
                output_type: node.value_type.clone(),
                root: node.clone(),
            };
            PreparedOperation::Literal(one.evaluate(&canonical_zero(kind, literal)?)?)
        }
        PortableExpressionOperation::Unary { operator, operand } => PreparedOperation::Unary {
            operator: *operator,
            operand: Box::new(prepare_node(operand)?),
        },
        PortableExpressionOperation::Binary {
            operator,
            left,
            right,
        } => PreparedOperation::Binary {
            operator: *operator,
            left: Box::new(prepare_node(left)?),
            right: Box::new(prepare_node(right)?),
        },
        PortableExpressionOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => PreparedOperation::Conditional {
            condition: Box::new(prepare_node(condition)?),
            when_true: Box::new(prepare_node(when_true)?),
            when_false: Box::new(prepare_node(when_false)?),
        },
        _ => {
            return Err(Refusal::UnsupportedType(
                "structured expression runtime".into(),
            ))
        }
    };
    Ok(PreparedNode { kind, operation })
}

// Literal evaluation ignores the program input, but the allocating conformance
// entrance still validates it. Supply one valid value of the exact leaf kind.
fn canonical_zero(kind: PrimitiveInfoKind, literal: &str) -> Result<Vec<u8>, Refusal> {
    let value = match kind {
        PrimitiveInfoKind::Bool => InfoBool::FALSE.encode().to_vec(),
        PrimitiveInfoKind::Text => Vec::new(),
        PrimitiveInfoKind::Count => encode_count(0).to_vec(),
        PrimitiveInfoKind::Scalar => Scalar::from_raw_microunits(0).encode().to_vec(),
        kind if quantity_kind(kind) => Quantity::new(0, quantity_unit(kind)).encode().to_vec(),
        kind if fixed_integer(kind) => {
            let encoded = FixedInteger::from_unsigned(kind, 0)
                .map_err(|_| Refusal::InvalidLiteral)?
                .encode();
            encoded.0[..encoded.1].to_vec()
        }
        _ => return Err(Refusal::UnsupportedType(literal.to_string())),
    };
    Ok(value)
}

fn evaluate_node(
    node: &PreparedNode,
    input: &[u8],
    input_kind: PrimitiveInfoKind,
) -> Result<PrimitiveValue, Refusal> {
    let value = match &node.operation {
        PreparedOperation::Input => {
            if node.kind != input_kind {
                return Err(Refusal::InvalidProgram);
            }
            PrimitiveValue::new(node.kind, input)?
        }
        PreparedOperation::Literal(encoded) => PrimitiveValue::new(node.kind, encoded)?,
        PreparedOperation::Unary { operator, operand } => {
            let operand = evaluate_node(operand, input, input_kind)?;
            evaluate_unary(*operator, node.kind, &operand)?
        }
        PreparedOperation::Binary {
            operator,
            left,
            right,
        } => {
            let left = evaluate_node(left, input, input_kind)?;
            let right = evaluate_node(right, input, input_kind)?;
            evaluate_binary(*operator, node.kind, &left, &right)?
        }
        PreparedOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            let condition = evaluate_node(condition, input, input_kind)?;
            let selected = if decode_bool(&condition)? {
                when_true
            } else {
                when_false
            };
            evaluate_node(selected, input, input_kind)?
        }
    };
    if value.kind == node.kind {
        Ok(value)
    } else {
        Err(Refusal::InvalidProgram)
    }
}

fn evaluate_unary(
    operator: UnaryOperator,
    expected: PrimitiveInfoKind,
    operand: &PrimitiveValue,
) -> Result<PrimitiveValue, Refusal> {
    match operator {
        UnaryOperator::Not => {
            PrimitiveValue::new(expected, &InfoBool::new(!decode_bool(operand)?).encode())
        }
        UnaryOperator::Negate if operand.kind == PrimitiveInfoKind::Scalar => {
            let encoded = Scalar::decode(operand.as_slice())
                .map_err(|_| Refusal::InvalidProgram)?
                .checked_neg()
                .map_err(|_| Refusal::Arithmetic)?
                .encode();
            PrimitiveValue::new(expected, &encoded)
        }
        UnaryOperator::Negate => {
            let encoded = decode_integer(operand)?
                .checked_neg()
                .map_err(|_| Refusal::Arithmetic)?
                .encode();
            PrimitiveValue::new(expected, &encoded.0[..encoded.1])
        }
    }
}

fn evaluate_binary(
    operator: BinaryOperator,
    expected: PrimitiveInfoKind,
    left: &PrimitiveValue,
    right: &PrimitiveValue,
) -> Result<PrimitiveValue, Refusal> {
    if left.kind != right.kind {
        return Err(Refusal::InvalidProgram);
    }
    if matches!(
        operator,
        BinaryOperator::BooleanAnd | BinaryOperator::BooleanOr
    ) {
        let value = match operator {
            BinaryOperator::BooleanAnd => decode_bool(left)? && decode_bool(right)?,
            BinaryOperator::BooleanOr => decode_bool(left)? || decode_bool(right)?,
            _ => unreachable!(),
        };
        return PrimitiveValue::new(expected, &InfoBool::new(value).encode());
    }
    if matches!(
        operator,
        BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Equal
            | BinaryOperator::NotEqual
    ) {
        let ordering = compare(left, right)?;
        let value = match operator {
            BinaryOperator::Less => ordering.is_lt(),
            BinaryOperator::LessOrEqual => !ordering.is_gt(),
            BinaryOperator::Greater => ordering.is_gt(),
            BinaryOperator::GreaterOrEqual => !ordering.is_lt(),
            BinaryOperator::Equal => ordering.is_eq(),
            BinaryOperator::NotEqual => !ordering.is_eq(),
            _ => unreachable!(),
        };
        return PrimitiveValue::new(expected, &InfoBool::new(value).encode());
    }
    if left.kind == PrimitiveInfoKind::Count {
        let left = decode_count(left.as_slice()).map_err(|_| Refusal::InvalidProgram)?;
        let right = decode_count(right.as_slice()).map_err(|_| Refusal::InvalidProgram)?;
        let result = match operator {
            BinaryOperator::Multiply => left.checked_mul(right),
            BinaryOperator::Divide if right != 0 => left.checked_div(right),
            BinaryOperator::Remainder if right != 0 => left.checked_rem(right),
            BinaryOperator::Add => left.checked_add(right),
            BinaryOperator::Subtract => left.checked_sub(right),
            _ => None,
        }
        .ok_or(Refusal::Arithmetic)?;
        return PrimitiveValue::new(expected, &encode_count(result));
    }
    if left.kind == PrimitiveInfoKind::Scalar {
        let left = Scalar::decode(left.as_slice()).map_err(|_| Refusal::InvalidProgram)?;
        let right = Scalar::decode(right.as_slice()).map_err(|_| Refusal::InvalidProgram)?;
        let result = match operator {
            BinaryOperator::Multiply => left.checked_mul(right),
            BinaryOperator::Divide => left.checked_div(right),
            BinaryOperator::Remainder => left.checked_rem(right),
            BinaryOperator::Add => left.checked_add(right),
            BinaryOperator::Subtract => left.checked_sub(right),
            _ => return Err(Refusal::InvalidProgram),
        }
        .map_err(|_| Refusal::Arithmetic)?;
        return PrimitiveValue::new(expected, &result.encode());
    }
    let left = decode_integer(left)?;
    let right = decode_integer(right)?;
    let result = match operator {
        BinaryOperator::Multiply => left.checked_mul(right),
        BinaryOperator::Divide => left.checked_div(right),
        BinaryOperator::Remainder => left.checked_rem(right),
        BinaryOperator::Add => left.checked_add(right),
        BinaryOperator::Subtract => left.checked_sub(right),
        BinaryOperator::BitAnd => left.bit_and(right),
        BinaryOperator::BitXor => left.bit_xor(right),
        BinaryOperator::BitOr => left.bit_or(right),
        BinaryOperator::ShiftLeft => left.checked_shift_left(shift_count(right)?),
        BinaryOperator::ShiftRight => left.checked_shift_right(shift_count(right)?),
        _ => return Err(Refusal::InvalidProgram),
    }
    .map_err(|_| Refusal::Arithmetic)?;
    let encoded = result.encode();
    PrimitiveValue::new(expected, &encoded.0[..encoded.1])
}

fn compare(left: &PrimitiveValue, right: &PrimitiveValue) -> Result<Ordering, Refusal> {
    match left.kind {
        PrimitiveInfoKind::Bool | PrimitiveInfoKind::Text => {
            Ok(left.as_slice().cmp(right.as_slice()))
        }
        PrimitiveInfoKind::Count => Ok(decode_count(left.as_slice())
            .map_err(|_| Refusal::InvalidProgram)?
            .cmp(&decode_count(right.as_slice()).map_err(|_| Refusal::InvalidProgram)?)),
        PrimitiveInfoKind::Scalar => Ok(Scalar::decode(left.as_slice())
            .map_err(|_| Refusal::InvalidProgram)?
            .cmp(&Scalar::decode(right.as_slice()).map_err(|_| Refusal::InvalidProgram)?)),
        kind if quantity_kind(kind) => Quantity::decode(left.as_slice())
            .map_err(|_| Refusal::InvalidProgram)?
            .compare(Quantity::decode(right.as_slice()).map_err(|_| Refusal::InvalidProgram)?)
            .map_err(|_| Refusal::Arithmetic),
        kind if fixed_integer(kind) => decode_integer(left)?
            .compare(decode_integer(right)?)
            .map_err(|_| Refusal::InvalidProgram),
        _ => Err(Refusal::UnsupportedType(kind_name(left.kind).into())),
    }
}

fn decode_bool(value: &PrimitiveValue) -> Result<bool, Refusal> {
    if value.kind != PrimitiveInfoKind::Bool {
        return Err(Refusal::InvalidProgram);
    }
    InfoBool::decode(value.as_slice())
        .map(InfoBool::get)
        .map_err(|_| Refusal::InvalidProgram)
}

fn decode_integer(value: &PrimitiveValue) -> Result<FixedInteger, Refusal> {
    if !fixed_integer(value.kind) {
        return Err(Refusal::InvalidProgram);
    }
    FixedInteger::decode(value.kind, value.as_slice()).map_err(|_| Refusal::InvalidProgram)
}

fn shift_count(value: FixedInteger) -> Result<u32, Refusal> {
    let value = if signed_integer(value.kind()) {
        u128::try_from(value.signed().map_err(|_| Refusal::Arithmetic)?)
            .map_err(|_| Refusal::Arithmetic)?
    } else {
        value.unsigned().map_err(|_| Refusal::Arithmetic)?
    };
    u32::try_from(value).map_err(|_| Refusal::Arithmetic)
}

fn leaf_kind(value_type: &conduit_core::StructuredInfoType) -> Result<PrimitiveInfoKind, Refusal> {
    let StructuredInfoTypeShape::Leaf(kind) = value_type.shape() else {
        return Err(Refusal::UnsupportedType(
            "structured expression runtime".into(),
        ));
    };
    primitive_info_kind(kind.as_str()).ok_or_else(|| Refusal::UnsupportedType(kind.as_str().into()))
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

const fn quantity_unit(kind: PrimitiveInfoKind) -> QuantityUnit {
    match kind {
        PrimitiveInfoKind::Quantity => QuantityUnit::One,
        PrimitiveInfoKind::Distance => QuantityUnit::Millimeter,
        PrimitiveInfoKind::Frequency => QuantityUnit::Hertz,
        PrimitiveInfoKind::Duration => QuantityUnit::Millisecond,
        PrimitiveInfoKind::Voltage => QuantityUnit::Volt,
        PrimitiveInfoKind::Temperature => QuantityUnit::Celsius,
        PrimitiveInfoKind::Angle => QuantityUnit::Degree,
        PrimitiveInfoKind::Ratio => QuantityUnit::Percent,
        PrimitiveInfoKind::PixelCount => QuantityUnit::Pixel,
        _ => QuantityUnit::One,
    }
}

const fn kind_name(kind: PrimitiveInfoKind) -> &'static str {
    match kind {
        PrimitiveInfoKind::Bool => BOOL_INFO_ID,
        PrimitiveInfoKind::Text => conduit_core::TEXT_INFO_ID,
        PrimitiveInfoKind::Count => COUNT_INFO_ID,
        PrimitiveInfoKind::Scalar => SCALAR_INFO_ID,
        PrimitiveInfoKind::U8 => "value/u8",
        PrimitiveInfoKind::U16 => "value/u16",
        PrimitiveInfoKind::U32 => "value/u32",
        PrimitiveInfoKind::U64 => "value/u64",
        PrimitiveInfoKind::U128 => "value/u128",
        PrimitiveInfoKind::I8 => "value/i8",
        PrimitiveInfoKind::I16 => "value/i16",
        PrimitiveInfoKind::I32 => "value/i32",
        PrimitiveInfoKind::I64 => "value/i64",
        PrimitiveInfoKind::I128 => "value/i128",
        PrimitiveInfoKind::Quantity => conduit_core::QUANTITY_INFO_ID,
        PrimitiveInfoKind::Distance => conduit_core::DISTANCE_INFO_ID,
        PrimitiveInfoKind::Frequency => conduit_core::FREQUENCY_INFO_ID,
        PrimitiveInfoKind::Duration => conduit_core::DURATION_INFO_ID,
        PrimitiveInfoKind::Voltage => conduit_core::VOLTAGE_INFO_ID,
        PrimitiveInfoKind::Temperature => conduit_core::TEMPERATURE_INFO_ID,
        PrimitiveInfoKind::Angle => conduit_core::ANGLE_INFO_ID,
        PrimitiveInfoKind::Ratio => conduit_core::RATIO_INFO_ID,
        PrimitiveInfoKind::PixelCount => conduit_core::PIXEL_COUNT_INFO_ID,
        _ => "unsupported",
    }
}
