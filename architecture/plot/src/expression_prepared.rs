//! Pre-Play preparation and allocation-free evaluation for primitive expressions.

use crate::{
    BinaryOperator, PortableExpressionEvaluationRefusal as Refusal, PortableExpressionNode,
    PortableExpressionOperation, PortableExpressionProgram, UnaryOperator,
};
use alloc::{boxed::Box, vec::Vec};
use conduit_core::{
    primitive_info_kind, PrimitiveInfoKind, StructuredInfoTypeShape, BOOL_INFO_ID, COUNT_INFO_ID,
    SCALAR_INFO_ID,
};

mod inspection;
mod member_selection;
mod nominal;
use member_selection::PreparedMemberSelection;
mod primitive;
mod storage_bound;
mod structured;
mod structured_contract;
use primitive::{decode_bool, evaluate_binary, evaluate_unary, PrimitiveValue};
use structured::PreparedStructuredExpression;

/// A prepared primitive-only evaluator. Construction owns every allocation;
/// `evaluate` uses fixed stack values and one capacity-stable output buffer.
pub struct PreparedPortableExpressionEvaluator {
    root: PreparedRoot,
    input: PreparedInput,
    output: Vec<u8>,
}

enum PreparedInput {
    Primitive {
        kind: PrimitiveInfoKind,
        nominal_type: Option<Vec<u8>>,
    },
    Structured(Vec<u8>),
}

enum PreparedRoot {
    Primitive {
        node: PreparedNode,
        nominal_type: Option<Vec<u8>>,
    },
    Structured(PreparedStructuredExpression),
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
        proven: bool,
        left: Box<PreparedNode>,
        right: Box<PreparedNode>,
    },
    Conditional {
        condition: Box<PreparedNode>,
        when_true: Box<PreparedNode>,
        when_false: Box<PreparedNode>,
    },
    Projection(PreparedMemberSelection),
    Widen(Box<PreparedNode>),
    Inspection(inspection::PreparedInspection),
}

impl PreparedPortableExpressionEvaluator {
    pub fn new(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        let input = match program.input_type.shape() {
            StructuredInfoTypeShape::Leaf(_) => PreparedInput::Primitive {
                kind: leaf_kind(&program.input_type)?,
                nominal_type: None,
            },
            StructuredInfoTypeShape::Nominal { representation, .. }
                if matches!(representation.shape(), StructuredInfoTypeShape::Leaf(_)) =>
            {
                PreparedInput::Primitive {
                    kind: leaf_kind(&program.input_type)?,
                    nominal_type: Some(
                        program
                            .input_type
                            .canonical_bytes()
                            .map_err(|_| Refusal::InvalidProgram)?,
                    ),
                }
            }
            _ => PreparedInput::Structured(
                program
                    .input_type
                    .canonical_bytes()
                    .map_err(|_| Refusal::InvalidProgram)?,
            ),
        };
        let root = match program.output_type.shape() {
            StructuredInfoTypeShape::Leaf(_) | StructuredInfoTypeShape::Nominal { .. } => {
                let root = prepare_node(&program.root, &program.input_type)?;
                if root.kind != leaf_kind(&program.output_type)? {
                    return Err(Refusal::InvalidProgram);
                }
                PreparedRoot::Primitive {
                    node: root,
                    nominal_type: matches!(
                        program.output_type.shape(),
                        StructuredInfoTypeShape::Nominal { .. }
                    )
                    .then(|| {
                        program
                            .output_type
                            .canonical_bytes()
                            .map_err(|_| Refusal::InvalidProgram)
                    })
                    .transpose()?,
                }
            }
            _ => PreparedRoot::Structured(PreparedStructuredExpression::new(program)?),
        };
        Ok(Self {
            root,
            input,
            output: Vec::with_capacity(storage_bound::output(&program.output_type)?),
        })
    }

    pub fn evaluate(&mut self, input: &[u8]) -> Result<&[u8], Refusal> {
        let mut primitive_bytes = input;
        let primitive_input = match &self.input {
            PreparedInput::Primitive { kind, nominal_type } => {
                if let Some(expected) = nominal_type {
                    primitive_bytes = nominal::input_payload(input, expected)?;
                }
                conduit_core::validate_primitive_info(kind_name(*kind), primitive_bytes)
                    .map_err(|_| Refusal::InvalidInput)?;
                Some(*kind)
            }
            PreparedInput::Structured(expected) => {
                let validated = conduit_core::validate_canonical_structured_value(input)
                    .map_err(|_| Refusal::InvalidInput)?;
                if validated.type_bytes() != expected {
                    return Err(Refusal::InvalidInput);
                }
                None
            }
        };
        self.output.clear();
        match &mut self.root {
            PreparedRoot::Primitive { node, nominal_type } => {
                let value = evaluate_node(node, primitive_bytes, primitive_input)?;
                if let Some(value_type) = nominal_type {
                    nominal::append_output(&mut self.output, value_type, &value)?;
                } else {
                    if value.length > self.output.capacity() {
                        return Err(Refusal::InvalidProgram);
                    }
                    self.output.extend_from_slice(value.as_slice());
                }
            }
            PreparedRoot::Structured(root) => root.evaluate(input, &mut self.output)?,
        }
        Ok(&self.output)
    }

    pub fn output_capacity(&self) -> usize {
        self.output.capacity()
    }
}

fn prepare_node(
    node: &PortableExpressionNode,
    input_type: &conduit_core::StructuredInfoType,
) -> Result<PreparedNode, Refusal> {
    let kind = leaf_kind(&node.value_type)?;
    let operation = match &node.operation {
        PortableExpressionOperation::Input => PreparedOperation::Input,
        PortableExpressionOperation::Literal(literal) => PreparedOperation::Literal(
            crate::expression_evaluate::literal_primitive_bytes(&node.value_type, literal)?,
        ),
        PortableExpressionOperation::Unary { operator, operand } => PreparedOperation::Unary {
            operator: *operator,
            operand: Box::new(prepare_node(operand, input_type)?),
        },
        PortableExpressionOperation::Binary {
            operator,
            proven,
            left,
            right,
        } => PreparedOperation::Binary {
            operator: *operator,
            proven: *proven,
            left: Box::new(prepare_node(left, input_type)?),
            right: Box::new(prepare_node(right, input_type)?),
        },
        PortableExpressionOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => PreparedOperation::Conditional {
            condition: Box::new(prepare_node(condition, input_type)?),
            when_true: Box::new(prepare_node(when_true, input_type)?),
            when_false: Box::new(prepare_node(when_false, input_type)?),
        },
        PortableExpressionOperation::SemanticCall {
            kind: call,
            arguments,
        } if crate::expression_semantic_call::integer_widening_target(call).is_some() => {
            let [argument] = arguments.as_slice() else {
                return Err(Refusal::InvalidProgram);
            };
            let operand = prepare_node(argument, input_type)?;
            if call != kind_name(kind)
                || !crate::expression_semantic_call::is_strict_widening(
                    kind_name(operand.kind),
                    call,
                )
            {
                return Err(Refusal::InvalidProgram);
            }
            PreparedOperation::Widen(Box::new(operand))
        }
        PortableExpressionOperation::SemanticCall {
            kind: call,
            arguments,
        } if matches!(
            call.as_str(),
            "variant/is" | "variant/tag" | "sequence/length"
        ) =>
        {
            let expected = match call.as_str() {
                "variant/is" => PrimitiveInfoKind::Bool,
                "variant/tag" => PrimitiveInfoKind::Text,
                _ => PrimitiveInfoKind::U64,
            };
            if kind != expected {
                return Err(Refusal::InvalidProgram);
            }
            PreparedOperation::Inspection(inspection::PreparedInspection::new(
                call, arguments, input_type,
            )?)
        }
        PortableExpressionOperation::Projection { .. } => {
            PreparedOperation::Projection(member_selection::prepare(node)?)
        }
        _ => {
            return Err(Refusal::UnsupportedType(
                "structured expression runtime".into(),
            ))
        }
    };
    Ok(PreparedNode { kind, operation })
}

fn evaluate_node<'a>(
    node: &'a mut PreparedNode,
    input: &'a [u8],
    input_kind: Option<PrimitiveInfoKind>,
) -> Result<PrimitiveValue<'a>, Refusal> {
    let expected = node.kind;
    let value = match &mut node.operation {
        PreparedOperation::Input => {
            if Some(expected) != input_kind {
                return Err(Refusal::InvalidProgram);
            }
            PrimitiveValue::borrowed(expected, input)?
        }
        PreparedOperation::Literal(encoded) => PrimitiveValue::borrowed(expected, encoded)?,
        PreparedOperation::Unary { operator, operand } => {
            let operand = evaluate_node(operand, input, input_kind)?;
            evaluate_unary(*operator, expected, &operand)?
        }
        PreparedOperation::Binary {
            operator,
            proven,
            left,
            right,
        } => {
            let left = evaluate_node(left, input, input_kind)?;
            let right = evaluate_node(right, input, input_kind)?;
            evaluate_binary(*operator, *proven, expected, &left, &right)?
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
        PreparedOperation::Widen(operand) => {
            let operand = evaluate_node(operand, input, input_kind)?;
            primitive::evaluate_widen(expected, &operand)?
        }
        PreparedOperation::Inspection(inspection) => inspection.evaluate(input)?,
        PreparedOperation::Projection(projection) => {
            PrimitiveValue::borrowed(expected, projection.evaluate(input)?)?
        }
    };
    if value.kind == expected {
        Ok(value)
    } else {
        Err(Refusal::InvalidProgram)
    }
}

fn leaf_kind(value_type: &conduit_core::StructuredInfoType) -> Result<PrimitiveInfoKind, Refusal> {
    match value_type.shape() {
        StructuredInfoTypeShape::Leaf(kind) => primitive_info_kind(kind.as_str())
            .ok_or_else(|| Refusal::UnsupportedType(kind.as_str().into())),
        StructuredInfoTypeShape::Nominal { representation, .. } => leaf_kind(representation),
        _ => Err(Refusal::UnsupportedType(
            "structured expression runtime".into(),
        )),
    }
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

const fn kind_name(kind: PrimitiveInfoKind) -> &'static str {
    match kind {
        PrimitiveInfoKind::Unit => conduit_core::UNIT_INFO_ID,
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
