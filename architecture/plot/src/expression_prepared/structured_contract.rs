//! Refuse inconsistent structured constructors before prepared storage is exposed.
use super::Refusal;
use crate::{PortableExpressionNode, PortableExpressionOperation, PortableExpressionProgram};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};

pub(super) fn validate(program: &PortableExpressionProgram) -> Result<(), Refusal> {
    if program.root.value_type != program.output_type {
        return Err(Refusal::InvalidProgram);
    }
    match &program.root.operation {
        PortableExpressionOperation::Record(nodes) => {
            let StructuredInfoTypeShape::Record { fields, .. } = program.output_type.shape() else {
                return Err(Refusal::InvalidProgram);
            };
            if fields.len() != nodes.len() {
                return Err(Refusal::InvalidProgram);
            }
            for field in fields {
                let mut matching = nodes.iter().filter(|(name, _)| name == field.name());
                let (_, node) = matching.next().ok_or(Refusal::InvalidProgram)?;
                if matching.next().is_some() || node.value_type != *field.value_type() {
                    return Err(Refusal::InvalidProgram);
                }
            }
        }
        PortableExpressionOperation::Tuple(nodes) => {
            let StructuredInfoTypeShape::Record { fields, .. } = program.output_type.shape() else {
                return Err(Refusal::InvalidProgram);
            };
            if fields.len() != nodes.len() {
                return Err(Refusal::InvalidProgram);
            }
            for (index, (field, node)) in fields.iter().zip(nodes).enumerate() {
                if field.name() != alloc::format!("item-{index:05}")
                    || node.value_type != *field.value_type()
                {
                    return Err(Refusal::InvalidProgram);
                }
            }
        }
        PortableExpressionOperation::Collection(nodes) => {
            let (element, minimum, maximum) = match program.output_type.shape() {
                StructuredInfoTypeShape::Collection { element, length } => {
                    (element, length, length)
                }
                StructuredInfoTypeShape::Sequence {
                    element,
                    minimum_items,
                    maximum_items,
                } => (element, minimum_items, maximum_items),
                _ => return Err(Refusal::InvalidProgram),
            };
            if nodes.len() < usize::from(minimum) || nodes.len() > usize::from(maximum) {
                return Err(Refusal::InvalidProgram);
            }
            for node in nodes {
                exact(node, element)?;
            }
        }
        _ => (), // variant and conditional constructors validate their own children
    }
    Ok(())
}
fn exact(node: &PortableExpressionNode, expected: &StructuredInfoType) -> Result<(), Refusal> {
    if node.value_type == *expected {
        Ok(())
    } else {
        Err(Refusal::InvalidProgram)
    }
}
