//! Prepared member selection preserves exact canonical identity with finite scratch.
use super::{storage_bound, Refusal};
use crate::{PortableExpressionNode, PortableExpressionOperation};
use alloc::vec::Vec;
use conduit_core::{
    StructuredCanonicalSelection, StructuredInfoType, StructuredInfoTypeShape, StructuredSelector,
};

pub(super) struct PreparedMemberSelection {
    steps: Vec<PreparedMemberStep>,
    first: Vec<u8>,
    second: Vec<u8>,
    primitive: bool,
}

struct PreparedMemberStep {
    selector: StructuredSelector,
    input_type: Vec<u8>,
    output_type: Vec<u8>,
}

pub(super) fn prepare(node: &PortableExpressionNode) -> Result<PreparedMemberSelection, Refusal> {
    fn collect(
        node: &PortableExpressionNode,
        steps: &mut Vec<PreparedMemberStep>,
    ) -> Result<StructuredInfoType, Refusal> {
        match &node.operation {
            PortableExpressionOperation::Input => Ok(node.value_type.clone()),
            PortableExpressionOperation::Projection { value, member } => {
                let input_type = collect(value, steps)?;
                let selector = match member {
                    crate::PortableExpressionProjection::Field(field) => {
                        StructuredSelector::field(input_type.clone(), field.clone()).or_else(|_| {
                            StructuredSelector::variant(
                                input_type.clone(),
                                field.clone(),
                                conduit_core::UnmatchedVariantDisposition::Refuse,
                            )
                        })
                    }
                    crate::PortableExpressionProjection::TupleIndex(index) => {
                        StructuredSelector::index(input_type.clone(), *index).or_else(|_| {
                            StructuredSelector::field(
                                input_type.clone(),
                                alloc::format!("item-{index:05}"),
                            )
                        })
                    }
                }
                .map_err(|_| Refusal::InvalidProgram)?;
                if selector.output_type() != &node.value_type {
                    return Err(Refusal::InvalidProgram);
                }
                steps.push(PreparedMemberStep {
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
    let capacity = steps.iter().try_fold(0, |bound, step| {
        Ok::<_, Refusal>(
            bound
                .max(storage_bound::canonical(step.selector.input_type())?)
                .max(storage_bound::canonical(step.selector.output_type())?),
        )
    })?;
    if steps.is_empty() {
        return Err(Refusal::InvalidProgram);
    }
    Ok(PreparedMemberSelection {
        steps,
        first: Vec::with_capacity(capacity),
        second: Vec::with_capacity(capacity),
        primitive: is_primitive(&node.value_type),
    })
}

impl PreparedMemberSelection {
    pub(super) fn evaluate<'a>(&'a mut self, input: &[u8]) -> Result<&'a [u8], Refusal> {
        if input.len() > self.first.capacity() {
            return Err(Refusal::InvalidInput);
        }
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
        if !self.primitive {
            return Ok(&self.first);
        }
        let value = conduit_core::validate_canonical_structured_value(&self.first)
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

fn is_primitive(ty: &StructuredInfoType) -> bool {
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(_) => true,
        StructuredInfoTypeShape::Nominal { representation, .. } => is_primitive(representation),
        _ => false,
    }
}
