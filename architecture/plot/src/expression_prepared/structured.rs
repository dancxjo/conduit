//! Allocation-stable construction of anonymous structured expression results.
use super::EvaluationInput;

use super::{PreparedInput, PreparedPortableExpressionEvaluator, ProgramView, Refusal};
use crate::{PortableExpressionNode, PortableExpressionOperation, PortableExpressionProgram};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};

pub(super) struct PreparedStructuredExpression {
    type_prefix: Vec<u8>,
    shape: PreparedShape,
}

enum PreparedShape {
    Input,
    Constant(Vec<u8>),
    Selected(super::member_selection::PreparedMemberSelection),
    SequenceSelected(super::sequence_selection::PreparedSequenceSelection),
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

struct PreparedField {
    name: String,
    value: PreparedChild,
}

struct PreparedChild {
    value_type: StructuredInfoType,
    type_prefix: Vec<u8>,
    evaluator: PreparedPortableExpressionEvaluator,
}

impl PreparedStructuredExpression {
    pub(super) fn new(
        program: ProgramView<'_>,
        prepared_input: &PreparedInput,
    ) -> Result<Self, Refusal> {
        super::structured_contract::validate(program)?;
        let shape = match &program.root.operation {
            PortableExpressionOperation::Input if program.input_type == program.output_type => {
                PreparedShape::Input
            }
            PortableExpressionOperation::Literal(_) => {
                let constant = PortableExpressionProgram {
                    input_type: StructuredInfoType::leaf(conduit_core::kind_id(
                        conduit_core::UNIT_INFO_ID,
                    ))
                    .map_err(|_| Refusal::InvalidProgram)?,
                    output_type: program.output_type.clone(),
                    root: program.root.clone(),
                };
                PreparedShape::Constant(constant.evaluate(&[])?)
            }
            PortableExpressionOperation::Projection { .. } => PreparedShape::Selected(
                super::member_selection::prepare(program.root, program.input_type)?,
            ),
            PortableExpressionOperation::SemanticCall { kind, .. } if kind == "sequence/at" => {
                PreparedShape::SequenceSelected(
                    super::sequence_selection::PreparedSequenceSelection::new(
                        program.root,
                        program.input_type,
                        prepared_input,
                    )?,
                )
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
                ))
            }
        };
        Ok(Self {
            type_prefix: program
                .output_type
                .canonical_bytes()
                .map_err(|_| Refusal::InvalidProgram)?,
            shape,
        })
    }

    pub(super) fn evaluate(
        &mut self,
        input: EvaluationInput<'_>,
        output: &mut Vec<u8>,
    ) -> Result<(), Refusal> {
        match &mut self.shape {
            PreparedShape::Input => return append(output, input.bytes()),
            PreparedShape::Constant(bytes) => return append(output, bytes),
            PreparedShape::SequenceSelected(selection) => {
                return append(output, selection.evaluate(input)?)
            }
            PreparedShape::Selected(selection) => {
                return append(output, selection.evaluate(input)?)
            }
            _ => (),
        }
        if let PreparedShape::Conditional {
            condition,
            when_true,
            when_false,
        } = &mut self.shape
        {
            let selected = match condition.evaluator.evaluate_input(input)? {
                [1] => when_true,
                [0] => when_false,
                _ => return Err(Refusal::InvalidProgram),
            };
            append(output, selected.evaluator.evaluate_input(input)?)?;
            return Ok(());
        }
        append(output, &self.type_prefix)?;
        match &mut self.shape {
            PreparedShape::Input
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
    fn append_node(
        &mut self,
        input: EvaluationInput<'_>,
        output: &mut Vec<u8>,
    ) -> Result<(), Refusal> {
        let encoded = self.evaluator.evaluate_input(input)?;
        match self.value_type.shape() {
            StructuredInfoTypeShape::Leaf(_) => {
                push(output, 0)?;
                push_len(output, encoded.len())?;
                append(output, encoded)
            }
            _ => {
                let node = encoded
                    .strip_prefix(self.type_prefix.as_slice())
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
        input_type: program.input_type,
        output_type: &node.value_type,
        root: node,
    };
    Ok(PreparedChild {
        type_prefix: node
            .value_type
            .canonical_bytes()
            .map_err(|_| Refusal::InvalidProgram)?,
        value_type: node.value_type.clone(),
        evaluator: PreparedPortableExpressionEvaluator::prepare(child, prepared_input.clone())?,
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

impl PreparedStructuredExpression {
    pub(super) fn owned_heap_bytes(&self) -> usize {
        self.type_prefix
            .capacity()
            .saturating_add(match &self.shape {
                PreparedShape::Input => 0,
                PreparedShape::Constant(value) => value.capacity(),
                PreparedShape::Selected(value) => value.owned_heap_bytes(),
                PreparedShape::SequenceSelected(value) => value.owned_heap_bytes(),
                PreparedShape::Record(fields) => fields.iter().fold(
                    fields
                        .capacity()
                        .saturating_mul(core::mem::size_of::<PreparedField>()),
                    |total, field| {
                        total
                            .saturating_add(field.name.capacity())
                            .saturating_add(field.value.owned_heap_bytes())
                    },
                ),
                PreparedShape::Collection(values) => values.iter().fold(
                    values
                        .capacity()
                        .saturating_mul(core::mem::size_of::<PreparedChild>()),
                    |total, value| total.saturating_add(value.owned_heap_bytes()),
                ),
                PreparedShape::Variant { tag, payload } => tag.capacity().saturating_add(
                    super::storage::boxed(payload.as_ref(), payload.owned_heap_bytes()),
                ),
                PreparedShape::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => super::storage::boxed(condition.as_ref(), condition.owned_heap_bytes())
                    .saturating_add(super::storage::boxed(
                        when_true.as_ref(),
                        when_true.owned_heap_bytes(),
                    ))
                    .saturating_add(super::storage::boxed(
                        when_false.as_ref(),
                        when_false.owned_heap_bytes(),
                    )),
            })
    }
}
impl PreparedChild {
    fn owned_heap_bytes(&self) -> usize {
        self.value_type
            .owned_heap_bytes()
            .saturating_add(self.type_prefix.capacity())
            .saturating_add(self.evaluator.owned_heap_bytes())
    }
}
