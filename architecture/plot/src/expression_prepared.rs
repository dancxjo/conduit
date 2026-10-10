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

mod byte_observation;
mod equality;
mod inspection;
mod shared_input;
use shared_input::SharedBytes;
mod member_selection;
mod nominal;
mod sequence_selection;
use member_selection::PreparedMemberSelection;
mod primitive;
mod storage_bound;
mod structured;
mod structured_contract;
mod text_material;
use primitive::{decode_bool, evaluate_binary, evaluate_unary, PrimitiveValue};
use structured::PreparedStructuredExpression;

/// A prepared primitive-only evaluator. Construction owns every allocation;
/// `evaluate` uses fixed stack values and one capacity-stable output buffer.
pub struct PreparedPortableExpressionEvaluator {
    root: PreparedRoot,
    input: PreparedInput,
    output: Vec<u8>,
}

#[derive(Clone)]
enum PreparedInput {
    Primitive {
        kind: PrimitiveInfoKind,
        nominal_type: Option<SharedBytes>,
    },
    Structured(SharedBytes),
}

#[derive(Clone, Copy)]
struct ProgramView<'a> {
    input_type: &'a conduit_core::StructuredInfoType,
    output_type: &'a conduit_core::StructuredInfoType,
    root: &'a PortableExpressionNode,
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
    Equality(Box<equality::PreparedEquality>),
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
    SequenceSelection(sequence_selection::PreparedSequenceSelection),
    Widen(Box<PreparedNode>),
    TextMaterial(Box<PreparedNode>),
    Inspection(inspection::PreparedInspection),
    Bytes(byte_observation::PreparedByteObservation),
}

/// Finite canonical frame ceiling from an exact checked Type, including leaf framing.
pub fn maximum_prepared_canonical_value_bytes(
    ty: &conduit_core::StructuredInfoType,
) -> Result<u32, Refusal> {
    Ok(storage_bound::canonical(ty)? as u32)
}

impl PortableExpressionProgram {
    /// Conservative canonical transport ceiling, computed before Play from the exact input Type.
    pub fn maximum_prepared_input_bytes(&self) -> Result<u32, Refusal> {
        Ok(storage_bound::output(&self.input_type)? as u32)
    }

    /// Conservative canonical transport ceiling, computed before Play from the exact output Type.
    pub fn maximum_prepared_output_bytes(&self) -> Result<u32, Refusal> {
        Ok(storage_bound::output(&self.output_type)? as u32)
    }
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
                            .map_err(|_| Refusal::InvalidProgram)?
                            .into(),
                    ),
                }
            }
            _ => PreparedInput::Structured(
                program
                    .input_type
                    .canonical_bytes()
                    .map_err(|_| Refusal::InvalidProgram)?
                    .into(),
            ),
        };
        Self::prepare(
            ProgramView {
                input_type: &program.input_type,
                output_type: &program.output_type,
                root: &program.root,
            },
            input,
        )
    }

    fn prepare(program: ProgramView<'_>, input: PreparedInput) -> Result<Self, Refusal> {
        let root = match program.output_type.shape() {
            StructuredInfoTypeShape::Leaf(_) | StructuredInfoTypeShape::Nominal { .. }
                if leaf_kind(program.output_type).is_ok() =>
            {
                let root = prepare_node(program.root, program.input_type, &input)?;
                if root.kind != leaf_kind(program.output_type)? {
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
            _ => PreparedRoot::Structured(PreparedStructuredExpression::new(program, &input)?),
        };
        Ok(Self {
            root,
            input,
            output: Vec::with_capacity(storage_bound::output(program.output_type)?),
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
                if validated.type_bytes() != expected.as_ref() {
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
    prepared_input: &PreparedInput,
) -> Result<PreparedNode, Refusal> {
    let kind = leaf_kind(&node.value_type)?;
    let operation = match &node.operation {
        PortableExpressionOperation::Constant(value) => {
            if value.value_type() != &node.value_type {
                return Err(Refusal::InvalidProgram);
            }
            let conduit_core::StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                return Err(Refusal::InvalidProgram);
            };
            PreparedOperation::Literal(bytes.to_vec())
        }
        PortableExpressionOperation::Input => PreparedOperation::Input,
        PortableExpressionOperation::Literal(literal) => PreparedOperation::Literal(
            crate::expression_evaluate::literal_primitive_bytes(&node.value_type, literal)?,
        ),
        PortableExpressionOperation::Unary { operator, operand } => PreparedOperation::Unary {
            operator: *operator,
            operand: Box::new(prepare_node(operand, input_type, prepared_input)?),
        },
        PortableExpressionOperation::Binary {
            operator,
            proven,
            left,
            right,
        } => equality::binary(
            *operator,
            *proven,
            kind,
            left,
            right,
            input_type,
            prepared_input,
        )?,
        PortableExpressionOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => PreparedOperation::Conditional {
            condition: Box::new(prepare_node(condition, input_type, prepared_input)?),
            when_true: Box::new(prepare_node(when_true, input_type, prepared_input)?),
            when_false: Box::new(prepare_node(when_false, input_type, prepared_input)?),
        },
        PortableExpressionOperation::SemanticCall { kind, arguments }
            if kind == "text/material" =>
        {
            text_material::prepare(node, arguments, input_type, prepared_input)?
        }
        PortableExpressionOperation::SemanticCall {
            kind: call,
            arguments,
        } if crate::expression_semantic_call::integer_widening_target(call).is_some() => {
            let [argument] = arguments.as_slice() else {
                return Err(Refusal::InvalidProgram);
            };
            let operand = prepare_node(argument, input_type, prepared_input)?;
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
                call,
                arguments,
                input_type,
                prepared_input,
            )?)
        }
        PortableExpressionOperation::SemanticCall {
            kind: call,
            arguments,
        } if matches!(call.as_str(), "bytes/length" | "bytes/at") => {
            let expected = if call == "bytes/at" {
                PrimitiveInfoKind::U8
            } else {
                PrimitiveInfoKind::U64
            };
            if kind != expected {
                return Err(Refusal::InvalidProgram);
            }
            PreparedOperation::Bytes(byte_observation::PreparedByteObservation::new(
                call,
                arguments,
                input_type,
                prepared_input,
            )?)
        }
        PortableExpressionOperation::SemanticCall { kind: call, .. } if call == "sequence/at" => {
            PreparedOperation::SequenceSelection(
                sequence_selection::PreparedSequenceSelection::new(
                    node,
                    input_type,
                    prepared_input,
                )?,
            )
        }
        PortableExpressionOperation::Projection { .. } => {
            PreparedOperation::Projection(member_selection::prepare(node, input_type)?)
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
        PreparedOperation::Equality(equality) => equality.evaluate(input)?,
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
        PreparedOperation::TextMaterial(operand) => evaluate_node(operand, input, input_kind)?,
        PreparedOperation::Widen(operand) => {
            let operand = evaluate_node(operand, input, input_kind)?;
            primitive::evaluate_widen(expected, &operand)?
        }
        PreparedOperation::Inspection(inspection) => inspection.evaluate(input)?,
        PreparedOperation::Bytes(observation) => observation.evaluate(input, input_kind)?,
        PreparedOperation::SequenceSelection(selection) => {
            PrimitiveValue::borrowed(expected, selection.evaluate(input)?)?
        }
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
        PrimitiveInfoKind::F32 => conduit_core::F32_INFO_ID,
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
        PrimitiveInfoKind::ExactDecimalQuantity => conduit_core::EXACT_DECIMAL_QUANTITY_INFO_ID,
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
