//! Allocation-stable construction of anonymous structured expression results.

use super::{
    projection::{prepare_projection, PreparedProjection},
    PreparedPortableExpressionEvaluator, Refusal,
};
use crate::{PortableExpressionNode, PortableExpressionOperation, PortableExpressionProgram};
use alloc::{string::String, vec::Vec};
use conduit_core::{
    StructuredInfoType, StructuredInfoTypeShape, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub(super) struct PreparedStructuredExpression {
    type_prefix: Vec<u8>,
    shape: PreparedShape,
}

enum PreparedShape {
    Input,
    Projection(PreparedProjection),
    Record(Vec<PreparedField>),
    Collection(Vec<PreparedChild>),
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
    pub(super) fn new(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        if program.root.value_type != program.output_type {
            return Err(Refusal::InvalidProgram);
        }
        let shape = match &program.root.operation {
            PortableExpressionOperation::Input => {
                if program.input_type != program.output_type {
                    return Err(Refusal::InvalidProgram);
                }
                PreparedShape::Input
            }
            PortableExpressionOperation::Projection { .. } => {
                PreparedShape::Projection(prepare_projection(&program.root)?)
            }
            PortableExpressionOperation::Tuple(values) => {
                let fields = values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        Ok(PreparedField {
                            name: alloc::format!("item-{index:05}"),
                            value: child(program, value)?,
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
                            value: child(program, value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Refusal>>()?;
                fields.sort_by(|left, right| left.name.cmp(&right.name));
                PreparedShape::Record(fields)
            }
            PortableExpressionOperation::Collection(values) => PreparedShape::Collection(
                values
                    .iter()
                    .map(|value| child(program, value))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
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

    pub(super) fn evaluate(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), Refusal> {
        // Selection returns a complete canonical value. Never reinterpret a
        // selected node under a different output schema.
        match &mut self.shape {
            PreparedShape::Input => return append(output, input),
            PreparedShape::Projection(projection) => {
                return append(output, projection.evaluate(input)?);
            }
            _ => {}
        }
        append(output, &self.type_prefix)?;
        match &mut self.shape {
            PreparedShape::Input | PreparedShape::Projection(_) => unreachable!(),
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
        }
        Ok(())
    }
}

impl PreparedChild {
    fn append_node(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), Refusal> {
        let encoded = self.evaluator.evaluate(input)?;
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
    program: &PortableExpressionProgram,
    node: &PortableExpressionNode,
) -> Result<PreparedChild, Refusal> {
    let child = PortableExpressionProgram {
        input_type: program.input_type.clone(),
        output_type: node.value_type.clone(),
        root: node.clone(),
    };
    Ok(PreparedChild {
        type_prefix: node
            .value_type
            .canonical_bytes()
            .map_err(|_| Refusal::InvalidProgram)?,
        value_type: node.value_type.clone(),
        evaluator: PreparedPortableExpressionEvaluator::new(&child)?,
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
    if output.len() == MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        return Err(Refusal::InvalidProgram);
    }
    output.push(value);
    Ok(())
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> Result<(), Refusal> {
    if output
        .len()
        .checked_add(value.len())
        .is_none_or(|length| length > MAXIMUM_STRUCTURED_CANONICAL_BYTES)
    {
        return Err(Refusal::InvalidProgram);
    }
    output.extend_from_slice(value);
    Ok(())
}
