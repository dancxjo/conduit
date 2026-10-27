//! Allocation-stable construction of anonymous structured expression results.

use super::{PreparedInput, PreparedPortableExpressionEvaluator, ProgramView, Refusal};
use crate::{PortableExpressionNode, PortableExpressionOperation, PortableExpressionProgram};
use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};
#[path = "structured_tape.rs"]
pub(super) mod tape;

pub(super) struct PreparedStructuredExpression {
    pub(super) type_prefix: Cow<'static, [u8]>,
    pub(super) shape: PreparedShape,
}

pub(super) enum PreparedShape {
    Tape(Box<tape::StructuredTape>),
    Input,
    Constant(Cow<'static, [u8]>),
    Selected(Box<super::member_selection::PreparedMemberSelection>),
    SequenceSelected(Box<super::sequence_selection::PreparedSequenceSelection>),
    Record(Vec<PreparedField>),
    Collection(Vec<PreparedChild>),
    Variant {
        tag: String,
        payload: Box<PreparedChild>,
    },
    Conditional {
        condition: Box<PreparedChild>,
        when_true: Box<PreparedChild>,
        when_false: Box<PreparedChild>,
    },
}

pub(super) struct PreparedField {
    name: String,
    value: PreparedChild,
}

pub(super) struct PreparedChild {
    pub(super) output: ChildOutput,
    pub(super) evaluator: ChildEvaluator,
}
pub(super) enum ChildEvaluator {
    Constant { encoded: Cow<'static, [u8]> },
    Dynamic(Box<PreparedPortableExpressionEvaluator>),
    Scalar(Box<super::compact::ScalarChild>),
}
impl ChildEvaluator {
    fn compact_scalars(&mut self) {
        let Self::Dynamic(evaluator) = self else {
            return;
        };
        evaluator.compact_scalars();
        if evaluator.scalar_child_eligible() {
            let Self::Dynamic(evaluator) = core::mem::replace(
                self,
                Self::Constant {
                    encoded: Cow::Owned(Vec::new()),
                },
            ) else {
                unreachable!("preflighted dynamic child")
            };
            *self = Self::Scalar(Box::new(super::compact::ScalarChild::take(*evaluator)));
        }
    }

    fn evaluate(&mut self, input: &[u8]) -> Result<&[u8], Refusal> {
        match self {
            Self::Constant { encoded, .. } => Ok(encoded),
            Self::Dynamic(evaluator) => evaluator.evaluate(input),
            Self::Scalar(evaluator) => evaluator.evaluate(input),
        }
    }
    fn output_bytes(&self) -> &[u8] {
        match self {
            Self::Constant { encoded, .. } => encoded,
            Self::Dynamic(evaluator) => evaluator.output_bytes(),
            Self::Scalar(evaluator) => evaluator.output_bytes(),
        }
    }
    fn output_type_prefix(&self) -> Option<&[u8]> {
        match self {
            Self::Constant { .. } | Self::Scalar(_) => None,
            Self::Dynamic(evaluator) => evaluator.output_type_prefix(),
        }
    }
}
pub(super) enum ChildOutput {
    Leaf,
    Structured,
}

impl PreparedStructuredExpression {
    pub(super) fn compact_scalars(&mut self) {
        match &mut self.shape {
            PreparedShape::Tape(tape) => tape.compact_scalars(),
            PreparedShape::Record(fields) => {
                for field in fields {
                    field.value.evaluator.compact_scalars();
                }
            }
            PreparedShape::Collection(values) => {
                for value in values {
                    value.evaluator.compact_scalars();
                }
            }
            PreparedShape::Variant { payload, .. } => payload.evaluator.compact_scalars(),
            PreparedShape::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                condition.evaluator.compact_scalars();
                when_true.evaluator.compact_scalars();
                when_false.evaluator.compact_scalars();
            }
            _ => (),
        }
    }

    pub(super) fn type_prefix(&self) -> &[u8] {
        &self.type_prefix
    }
    pub(super) fn new(
        program: ProgramView<'_>,
        prepared_input: &PreparedInput,
    ) -> Result<Self, Refusal> {
        super::structured_contract::validate(program)?;
        if let Some(tape) = tape::StructuredTape::prepare(program, prepared_input) {
            return Ok(Self {
                type_prefix: program
                    .output_type
                    .canonical_bytes()
                    .map_err(|_| Refusal::InvalidProgram)?
                    .into(),
                shape: PreparedShape::Tape(Box::new(tape)),
            });
        }
        let shape = match &program.root.operation {
            PortableExpressionOperation::Input if program.input_type == program.output_type => {
                PreparedShape::Input
            }
            PortableExpressionOperation::Literal(_) | PortableExpressionOperation::Constant(_) => {
                let constant = PortableExpressionProgram {
                    input_maximum_bytes: None,
                    input_type: StructuredInfoType::leaf(conduit_core::kind_id(
                        conduit_core::EMPTY_INFO_ID,
                    ))
                    .map_err(|_| Refusal::InvalidProgram)?,
                    output_type: program.output_type.clone(),
                    root: program.root.clone(),
                };
                PreparedShape::Constant(constant.evaluate(&[])?.into())
            }
            PortableExpressionOperation::Projection { .. } => PreparedShape::Selected(Box::new(
                super::member_selection::prepare(program.root, program.input_type, prepared_input)?,
            )),
            PortableExpressionOperation::SemanticCall { kind, .. } if kind == "sequence/at" => {
                PreparedShape::SequenceSelected(Box::new(
                    super::sequence_selection::PreparedSequenceSelection::new(
                        program.root,
                        program.input_type,
                        prepared_input,
                    )?,
                ))
            }
            PortableExpressionOperation::Tuple(values) => {
                let fields = values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        Ok(PreparedField {
                            name: alloc::format!("item-{index:05}"),
                            value: child(program, value, prepared_input)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Refusal>>()?;
                PreparedShape::Record(fields)
            }
            PortableExpressionOperation::Record(values) => {
                let mut fields = values
                    .iter()
                    .map(|(name, value)| {
                        Ok(PreparedField {
                            name: name.clone(),
                            value: child(program, value, prepared_input)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Refusal>>()?;
                fields.sort_by(|left, right| left.name.cmp(&right.name));
                PreparedShape::Record(fields)
            }
            PortableExpressionOperation::Collection(values) => PreparedShape::Collection(
                values
                    .iter()
                    .map(|value| child(program, value, prepared_input))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            PortableExpressionOperation::Variant { tag, payload } => {
                let StructuredInfoTypeShape::Variant { cases, .. } = program.output_type.shape()
                else {
                    return Err(Refusal::InvalidProgram);
                };
                let case = cases
                    .iter()
                    .find(|case| case.tag() == tag)
                    .ok_or(Refusal::InvalidProgram)?;
                if case.payload_type() != &payload.value_type {
                    return Err(Refusal::InvalidProgram);
                }
                PreparedShape::Variant {
                    tag: tag.clone(),
                    payload: Box::new(child(program, payload, prepared_input)?),
                }
            }
            PortableExpressionOperation::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                if condition.value_type
                    != StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID))
                        .map_err(|_| Refusal::InvalidProgram)?
                    || &when_true.value_type != program.output_type
                    || &when_false.value_type != program.output_type
                {
                    return Err(Refusal::InvalidProgram);
                }
                PreparedShape::Conditional {
                    condition: Box::new(child(program, condition, prepared_input)?),
                    when_true: Box::new(child(program, when_true, prepared_input)?),
                    when_false: Box::new(child(program, when_false, prepared_input)?),
                }
            }
            _ => {
                return Err(Refusal::UnsupportedType(
                    "structured expression runtime".into(),
                ));
            }
        };
        Ok(Self {
            type_prefix: program
                .output_type
                .canonical_bytes()
                .map_err(|_| Refusal::InvalidProgram)?
                .into(),
            shape,
        })
    }

    pub(super) fn evaluate(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), Refusal> {
        match &mut self.shape {
            PreparedShape::Tape(tape) => {
                append(output, &self.type_prefix)?;
                return tape.evaluate(input, output);
            }
            PreparedShape::Input => return append(output, input),
            PreparedShape::Constant(bytes) => return append(output, bytes),
            PreparedShape::SequenceSelected(selection) => {
                return append(output, selection.evaluate(input)?);
            }
            PreparedShape::Selected(selection) => {
                return append(output, selection.evaluate(input)?);
            }
            _ => (),
        }
        if let PreparedShape::Conditional {
            condition,
            when_true,
            when_false,
        } = &mut self.shape
        {
            let selected = match condition.evaluator.evaluate(input)? {
                [1] => when_true,
                [0] => when_false,
                _ => return Err(Refusal::InvalidProgram),
            };
            append(output, selected.evaluator.evaluate(input)?)?;
            return Ok(());
        }
        append(output, &self.type_prefix)?;
        match &mut self.shape {
            PreparedShape::Input
            | PreparedShape::Tape(_)
            | PreparedShape::Constant(_)
            | PreparedShape::Selected(_)
            | PreparedShape::SequenceSelected(_) => {
                unreachable!("identity selection handled before prefix")
            }
            PreparedShape::Record(fields) => {
                push(output, 2)?;
                push_len(output, fields.len())?;
                for field in fields {
                    push_text(output, &field.name)?;
                    field.value.append_node(input, output)?;
                }
            }
            PreparedShape::Collection(values) => {
                push(output, 1)?;
                push_len(output, values.len())?;
                for value in values {
                    value.append_node(input, output)?;
                }
            }
            PreparedShape::Variant { tag, payload } => {
                push(output, 3)?;
                push_text(output, tag)?;
                payload.append_node(input, output)?;
            }
            PreparedShape::Conditional { .. } => {
                unreachable!("conditional handled before canonical prefix")
            }
        }
        Ok(())
    }
}

impl PreparedChild {
    fn append_node(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), Refusal> {
        self.evaluator.evaluate(input)?;
        let encoded = self.evaluator.output_bytes();
        match &self.output {
            ChildOutput::Leaf => {
                push(output, 0)?;
                push_len(output, encoded.len())?;
                append(output, encoded)
            }
            ChildOutput::Structured => {
                let type_prefix = self
                    .evaluator
                    .output_type_prefix()
                    .ok_or(Refusal::InvalidProgram)?;
                let node = encoded
                    .strip_prefix(type_prefix)
                    .ok_or(Refusal::InvalidProgram)?;
                append(output, node)
            }
        }
    }
}

fn child(
    program: ProgramView<'_>,
    node: &PortableExpressionNode,
    prepared_input: &PreparedInput,
) -> Result<PreparedChild, Refusal> {
    let child = ProgramView {
        input_maximum_bytes: program.input_maximum_bytes,
        input_type: program.input_type,
        output_type: &node.value_type,
        root: node,
    };
    // Preserve independent canonical Type validation during preparation, then
    // compare with the evaluator's immutable prefix before releasing this copy.
    let type_prefix = node
        .value_type
        .canonical_bytes()
        .map_err(|_| Refusal::InvalidProgram)?;
    let leaf = matches!(node.value_type.shape(), StructuredInfoTypeShape::Leaf(_));
    let expected_prefix = if leaf {
        drop(type_prefix);
        None
    } else {
        Some(type_prefix)
    };
    let evaluator = match constant_child(node) {
        Some(constant) => constant,
        None => ChildEvaluator::Dynamic(Box::new(PreparedPortableExpressionEvaluator::prepare(
            child,
            prepared_input.clone(),
        )?)),
    };
    if let Some(expected) = expected_prefix {
        if evaluator.output_type_prefix() != Some(expected.as_slice()) {
            return Err(Refusal::InvalidProgram);
        }
    }
    Ok(PreparedChild {
        output: if leaf {
            ChildOutput::Leaf
        } else {
            ChildOutput::Structured
        },
        evaluator,
    })
}

/// Cache intrinsic fixed-integer/Boolean leaves using the existing literal codec.
/// Other Types, predicates and expression operations retain normal preparation.
/// Codec refusal falls back without changing its original refusal phase.
fn constant_child(node: &PortableExpressionNode) -> Option<ChildEvaluator> {
    if !matches!(node.value_type.shape(), StructuredInfoTypeShape::Leaf(_))
        || !matches!(
            super::leaf_kind(&node.value_type).ok()?,
            conduit_core::PrimitiveInfoKind::Bool
                | conduit_core::PrimitiveInfoKind::U8
                | conduit_core::PrimitiveInfoKind::U16
                | conduit_core::PrimitiveInfoKind::U32
                | conduit_core::PrimitiveInfoKind::U64
                | conduit_core::PrimitiveInfoKind::U128
                | conduit_core::PrimitiveInfoKind::I8
                | conduit_core::PrimitiveInfoKind::I16
                | conduit_core::PrimitiveInfoKind::I32
                | conduit_core::PrimitiveInfoKind::I64
                | conduit_core::PrimitiveInfoKind::I128
        )
    {
        return None;
    }
    let encoded = match &node.operation {
        PortableExpressionOperation::Literal(literal) => {
            crate::expression_evaluate::literal_primitive_bytes(&node.value_type, literal).ok()?
        }
        PortableExpressionOperation::Constant(value) if value.value_type() == &node.value_type => {
            let conduit_core::StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                return None;
            };
            bytes.to_vec()
        }
        _ => return None,
    };
    Some(ChildEvaluator::Constant {
        encoded: encoded.into(),
    })
}

fn push_text(output: &mut Vec<u8>, value: &str) -> Result<(), Refusal> {
    push_len(output, value.len())?;
    append(output, value.as_bytes())
}

fn push_len(output: &mut Vec<u8>, length: usize) -> Result<(), Refusal> {
    let length = u32::try_from(length).map_err(|_| Refusal::InvalidProgram)?;
    append(output, &length.to_le_bytes())
}

fn push(output: &mut Vec<u8>, value: u8) -> Result<(), Refusal> {
    if output.len() == output.capacity() {
        return Err(Refusal::InvalidProgram);
    }
    output.push(value);
    Ok(())
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> Result<(), Refusal> {
    if output
        .len()
        .checked_add(value.len())
        .is_none_or(|length| length > output.capacity())
    {
        return Err(Refusal::InvalidProgram);
    }
    output.extend_from_slice(value);
    Ok(())
}
