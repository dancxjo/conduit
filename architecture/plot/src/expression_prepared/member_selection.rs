//! Borrow nested exact members; only a structured final result needs scratch.
use super::{
    PreparedInput, PreparedPortableExpressionEvaluator, ProgramView, Refusal, storage_bound,
};
use crate::{PortableExpressionNode, PortableExpressionOperation, PortableExpressionProjection};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};

pub(super) struct PreparedMemberSelection {
    source: Option<Box<PreparedPortableExpressionEvaluator>>,
    steps: Vec<Member>,
    output_type: Vec<u8>,
    output: Vec<u8>,
    primitive: bool,
}
enum Member {
    Field(String),
    Index(u16),
    Variant(String),
}

pub(super) fn prepare(
    node: &PortableExpressionNode,
    input: &StructuredInfoType,
    prepared_input: &PreparedInput,
) -> Result<PreparedMemberSelection, Refusal> {
    fn collect<'a>(
        node: &'a PortableExpressionNode,
        input: &StructuredInfoType,
        steps: &mut Vec<Member>,
        base: &mut Option<&'a PortableExpressionNode>,
    ) -> Result<&'a StructuredInfoType, Refusal> {
        match &node.operation {
            PortableExpressionOperation::Input if &node.value_type == input => Ok(&node.value_type),
            PortableExpressionOperation::Projection { value, member } => {
                let source = collect(value, input, steps, base)?;
                let (selected, step) = match (source.shape(), member) {
                    (
                        StructuredInfoTypeShape::Record { fields, .. },
                        PortableExpressionProjection::Field(name),
                    ) => {
                        let field = fields
                            .iter()
                            .find(|field| field.name() == name)
                            .ok_or(Refusal::InvalidProgram)?;
                        (field.value_type(), Member::Field(name.clone()))
                    }
                    (
                        StructuredInfoTypeShape::Record { fields, .. },
                        PortableExpressionProjection::TupleIndex(index),
                    ) => {
                        let name = alloc::format!("item-{index:05}");
                        let field = fields
                            .iter()
                            .find(|field| field.name() == name)
                            .ok_or(Refusal::InvalidProgram)?;
                        (field.value_type(), Member::Field(name))
                    }
                    (
                        StructuredInfoTypeShape::Variant { cases, .. },
                        PortableExpressionProjection::Field(name),
                    ) => {
                        let case = cases
                            .iter()
                            .find(|case| case.tag() == name)
                            .ok_or(Refusal::InvalidProgram)?;
                        (case.payload_type(), Member::Variant(name.clone()))
                    }
                    (
                        StructuredInfoTypeShape::Collection { element, length },
                        PortableExpressionProjection::TupleIndex(index),
                    ) if *index < length => (element, Member::Index(*index)),
                    (
                        StructuredInfoTypeShape::Sequence {
                            element,
                            maximum_items,
                            ..
                        },
                        PortableExpressionProjection::TupleIndex(index),
                    ) if *index < maximum_items => (element, Member::Index(*index)),
                    _ => return Err(Refusal::InvalidProgram),
                };
                if selected != &node.value_type {
                    return Err(Refusal::InvalidProgram);
                }
                steps.push(step);
                Ok(&node.value_type)
            }
            PortableExpressionOperation::SemanticCall { kind, .. } if kind == "sequence/at" => {
                *base = Some(node);
                Ok(&node.value_type)
            }
            _ if !is_primitive(&node.value_type) => {
                // A projection may select from an admitted constructed value.
                // Preparing this exact source validates its constructor law.
                *base = Some(node);
                Ok(&node.value_type)
            }
            _ => Err(Refusal::InvalidProgram),
        }
    }
    let mut steps = Vec::new();
    let mut base = None;
    collect(node, input, &mut steps, &mut base)?;
    let source = base
        .map(|root| {
            PreparedPortableExpressionEvaluator::prepare(
                ProgramView {
                    input_maximum_bytes: prepared_input.maximum_bytes(),
                    input_type: input,
                    output_type: &root.value_type,
                    root,
                },
                prepared_input.clone(),
            )
            .map(Box::new)
        })
        .transpose()?;
    if steps.is_empty() {
        return Err(Refusal::InvalidProgram);
    }
    let primitive = is_primitive(&node.value_type);
    Ok(PreparedMemberSelection {
        source,
        steps,
        output_type: node
            .value_type
            .canonical_bytes()
            .map_err(|_| Refusal::InvalidProgram)?,
        output: Vec::with_capacity(if primitive {
            0
        } else {
            storage_bound::expression(node, input, prepared_input.maximum_bytes())?
        }),
        primitive,
    })
}
impl PreparedMemberSelection {
    pub(super) fn evaluate<'a>(&'a mut self, input: &'a [u8]) -> Result<&'a [u8], Refusal> {
        let selected = match &mut self.source {
            Some(source) => source.evaluate(input)?,
            None => input,
        };
        let mut value = conduit_core::validate_canonical_structured_value(selected)
            .map_err(|_| Refusal::InvalidInput)?;
        for step in &self.steps {
            value = match step {
                Member::Field(name) => value.record_field(name),
                Member::Index(index) => value.collection_index(*index),
                Member::Variant(name) => value.variant_payload(name),
            }
            .map_err(|_| Refusal::InvalidInput)?
            .ok_or(Refusal::InvalidInput)?;
        }
        render_selected(value, &self.output_type, self.primitive, &mut self.output)
    }
}

pub(super) fn render_selected<'a>(
    value: conduit_core::ValidatedCanonicalStructuredValue<'a>,
    output_type: &[u8],
    primitive: bool,
    output: &'a mut Vec<u8>,
) -> Result<&'a [u8], Refusal> {
    if value.type_bytes() != output_type {
        return Err(Refusal::InvalidInput);
    }
    let node = value.value_node();
    if primitive {
        let [0, length @ ..] = node else {
            return Err(Refusal::InvalidProgram);
        };
        let length_bytes = length.get(..4).ok_or(Refusal::InvalidProgram)?;
        let size = u32::from_le_bytes(length_bytes.try_into().unwrap()) as usize;
        let bytes = length.get(4..).ok_or(Refusal::InvalidProgram)?;
        if bytes.len() != size {
            return Err(Refusal::InvalidProgram);
        }
        return Ok(bytes);
    }
    if output_type
        .len()
        .checked_add(node.len())
        .is_none_or(|size| size > output.capacity())
    {
        return Err(Refusal::InvalidProgram);
    }
    output.clear();
    output.extend_from_slice(output_type);
    output.extend_from_slice(node);
    Ok(output.as_slice())
}

pub(super) fn is_primitive(ty: &StructuredInfoType) -> bool {
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(_) => true,
        StructuredInfoTypeShape::Nominal { representation, .. } => is_primitive(representation),
        _ => false,
    }
}
