//! Prepared exact-type projections shared by primitive and structured results.

use super::Refusal;
use crate::{PortableExpressionNode, PortableExpressionOperation};
use alloc::vec::Vec;
use conduit_core::{
    StructuredCanonicalSelection, StructuredInfoType, StructuredSelector,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub(super) struct PreparedProjection {
    steps: Vec<PreparedProjectionStep>,
    first: Vec<u8>,
    second: Vec<u8>,
}

struct PreparedProjectionStep {
    selector: StructuredSelector,
    input_type: Vec<u8>,
    output_type: Vec<u8>,
}

pub(super) fn prepare_projection(
    node: &PortableExpressionNode,
) -> Result<PreparedProjection, Refusal> {
    fn collect(
        node: &PortableExpressionNode,
        steps: &mut Vec<PreparedProjectionStep>,
    ) -> Result<StructuredInfoType, Refusal> {
        match &node.operation {
            PortableExpressionOperation::Input => Ok(node.value_type.clone()),
            PortableExpressionOperation::Projection { value, member } => {
                let input_type = collect(value, steps)?;
                let selector = match member {
                    crate::PortableExpressionProjection::Field(field) => {
                        StructuredSelector::field(input_type.clone(), field.clone())
                    }
                    crate::PortableExpressionProjection::TupleIndex(index) => {
                        StructuredSelector::field(
                            input_type.clone(),
                            alloc::format!("item-{index:05}"),
                        )
                    }
                }
                .map_err(|_| Refusal::InvalidProgram)?;
                if selector.output_type() != &node.value_type {
                    return Err(Refusal::InvalidProgram);
                }
                steps.push(PreparedProjectionStep {
                    input_type: selector
                        .input_type()
                        .canonical_bytes()
                        .map_err(|_| Refusal::InvalidProgram)?,
                    output_type: selector
                        .output_type()
                        .canonical_bytes()
                        .map_err(|_| Refusal::InvalidProgram)?,
                    selector,
                });
                Ok(node.value_type.clone())
            }
            _ => Err(Refusal::UnsupportedType(
                "projection must originate at the expression input".into(),
            )),
        }
    }

    let mut steps = Vec::new();
    collect(node, &mut steps)?;
    if steps.is_empty() {
        return Err(Refusal::InvalidProgram);
    }
    Ok(PreparedProjection {
        steps,
        first: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        second: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
    })
}

impl PreparedProjection {
    pub(super) fn evaluate<'a>(&'a mut self, input: &[u8]) -> Result<&'a [u8], Refusal> {
        self.first.clear();
        self.first.extend_from_slice(input);
        for step in &self.steps {
            self.second.clear();
            let selection = step
                .selector
                .select_canonical_into(
                    &self.first,
                    &step.input_type,
                    &step.output_type,
                    &mut self.second,
                )
                .map_err(|_| Refusal::InvalidInput)?;
            if selection != StructuredCanonicalSelection::Matched {
                return Err(Refusal::InvalidInput);
            }
            core::mem::swap(&mut self.first, &mut self.second);
        }
        Ok(&self.first)
    }

    pub(super) fn evaluate_primitive<'a>(&'a mut self, input: &[u8]) -> Result<&'a [u8], Refusal> {
        let encoded = self.evaluate(input)?;
        let value = conduit_core::validate_canonical_structured_value(encoded)
            .map_err(|_| Refusal::InvalidProgram)?
            .value_node();
        let [0, length @ ..] = value else {
            return Err(Refusal::InvalidProgram);
        };
        if length.len() < 4 {
            return Err(Refusal::InvalidProgram);
        }
        let encoded_length = u32::from_le_bytes(length[..4].try_into().unwrap()) as usize;
        let encoded = length.get(4..).ok_or(Refusal::InvalidProgram)?;
        if encoded.len() != encoded_length {
            return Err(Refusal::InvalidProgram);
        }
        Ok(encoded)
    }
}
