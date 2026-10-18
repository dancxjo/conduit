//! Portable semantic evaluation for one already checked expression program.

mod arithmetic;
mod equality;
use arithmetic::binary;
mod intrinsic;
mod literal;
mod structured;
use intrinsic::intrinsic_call;
pub(super) use literal::literal_primitive_bytes;
use literal::literal_value;
use structured::{encoded_structured, projection};

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
    QuantityProduct(conduit_core::RationalQuantityRefusal),
    QuantityProjection(conduit_core::ExactNumericRefusal),
    QuantityEncoding(conduit_core::QuantityRefusal),
    UnprovedMathematicalComparison,
    AngleTrigonometry(conduit_core::AngleTrigonometryRefusal),
    InverseAngle(conduit_core::InverseAngleEncodingRefusal),
    AngleReference(conduit_core::AngleReferenceRefusal),
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
        self.validate_input_bound()
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        if self
            .input_maximum_bytes
            .is_some_and(|maximum| input.len() > maximum as usize)
        {
            return Err(PortableExpressionEvaluationRefusal::InvalidInput);
        }
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
        PortableExpressionOperation::Constant(value) => {
            if value.value_type() != &node.value_type {
                return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
            }
            encoded_structured(value.clone())?
        }
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
        PortableExpressionOperation::CanonicalLiteral(bytes) => {
            validate_input(&node.value_type, bytes)?;
            Value {
                value_type: node.value_type.clone(),
                encoded: bytes.clone(),
            }
        }
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
            structured::tuple(values, node, input, input_type)?
        }
        PortableExpressionOperation::Record(fields) => {
            structured::record(fields, node, input, input_type)?
        }
        PortableExpressionOperation::Collection(values) => {
            structured::collection(values, node, input, input_type)?
        }
        PortableExpressionOperation::Variant { tag, payload } => {
            structured::variant(tag, payload, node, input, input_type)?
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

fn compare(left: &Value, right: &Value) -> Result<Ordering, PortableExpressionEvaluationRefusal> {
    let kind = leaf_kind(&left.value_type)?;
    if kind == conduit_core::F32_INFO_ID {
        crate::expression_float::binary32::ordering(
            &primitive_bytes(left)?,
            &primitive_bytes(right)?,
        )
        .ok_or(PortableExpressionEvaluationRefusal::Arithmetic)
    } else if kind == conduit_core::F64_INFO_ID {
        crate::expression_float::binary64::ordering(
            &primitive_bytes(left)?,
            &primitive_bytes(right)?,
        )
        .ok_or(PortableExpressionEvaluationRefusal::Arithmetic)
    } else if fixed_integer(
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
        || conduit_core::primitive_info_kind(kind)
            == Some(conduit_core::PrimitiveInfoKind::Quantity)
    {
        crate::expression_quantity::compare(&left.encoded, &right.encoded)
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
