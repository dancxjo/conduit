//! Borrow nested exact members; only a structured final result needs scratch.
use super::{PreparedInput, PreparedPortableExpressionEvaluator, ProgramView, Refusal};
use crate::{PortableExpressionNode, PortableExpressionOperation, PortableExpressionProjection};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};

pub(super) struct PreparedMemberSelection {
    source: Option<Box<PreparedPortableExpressionEvaluator>>,
    steps: Vec<Member>,
    output_type: Vec<u8>,
    primitive: bool,
    fixed: Option<(u32, u32)>,
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
    let output_type = node
        .value_type
        .canonical_bytes()
        .map_err(|_| Refusal::InvalidProgram)?;
    // The enclosing evaluator validates the complete exact canonical input.
    // Only direct fixed-width primitive selections can release their names and
    // output schema; variable layouts and constructed sources retain the path.
    let fixed = if primitive {
        match prepared_input {
            PreparedInput::Structured(prefix, _) => {
                super::fixed_projection::prepare(input, node, prefix.len())
            }
            _ => None,
        }
    } else {
        None
    };
    if fixed.is_some() {
        steps.clear();
        steps.shrink_to_fit();
    }
    Ok(PreparedMemberSelection {
        source,
        steps,
        output_type: if fixed.is_some() {
            Vec::new()
        } else {
            output_type
        },
        primitive,
        fixed,
    })
}
impl PreparedMemberSelection {
    pub(super) fn direct_input_path(
        &self,
    ) -> Option<Vec<super::resident_projection::OwnedExpressionPathStep>> {
        use super::resident_projection::OwnedExpressionPathStep as P;
        if self.source.is_some() || self.fixed.is_some() {
            return None;
        }
        Some(
            self.steps
                .iter()
                .map(|step| match step {
                    Member::Field(name) => P::Field(name.clone()),
                    Member::Index(index) => P::Index(*index),
                    Member::Variant(name) => P::Variant(name.clone()),
                })
                .collect(),
        )
    }

    /// Raw fixed leaf payload in an independently validated exact input.
    pub(super) fn fixed_scalar_input(&self) -> Option<(u32, u32)> {
        if self.source.is_some() || !self.primitive {
            return None;
        }
        let (start, length) = self.fixed?;
        Some((start.checked_add(5)?, length.checked_sub(5)?))
    }

    pub(super) fn evaluate<'a>(&'a mut self, input: &'a [u8]) -> Result<&'a [u8], Refusal> {
        if !self.primitive {
            return Err(Refusal::InvalidProgram);
        }
        if let Some((start, length)) = self.fixed {
            // Whole-input validation and checked geometry establish the leaf
            // framing. Still bound both the slice and its declared byte length.
            let node = input
                .get(start as usize..start as usize + length as usize)
                .ok_or(Refusal::InvalidInput)?;
            let [0, a, b, c, d, bytes @ ..] = node else {
                return Err(Refusal::InvalidInput);
            };
            if bytes.len() != u32::from_le_bytes([*a, *b, *c, *d]) as usize {
                return Err(Refusal::InvalidInput);
            }
            return Ok(bytes);
        }
        let value = select(&mut self.source, &self.steps, input)?;
        render_primitive(value, &self.output_type)
    }

    /// The enclosing evaluator already reserved the complete result.
    pub(super) fn evaluate_into(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<(), Refusal> {
        if self.primitive {
            // Opaque semantic leaves also use the structural evaluator. Keep
            // their raw-leaf framing without allocating a selection scratch.
            return super::structured::append(output, self.evaluate(input)?);
        }
        let value = select(&mut self.source, &self.steps, input)?;
        append_selected(value, &self.output_type, output)
    }
}

fn select<'a>(
    source: &'a mut Option<Box<PreparedPortableExpressionEvaluator>>,
    steps: &[Member],
    input: &'a [u8],
) -> Result<conduit_core::ValidatedCanonicalStructuredValue<'a>, Refusal> {
    let selected = match source {
        Some(source) => source.evaluate(input)?,
        None => input,
    };
    let mut value = conduit_core::validate_canonical_structured_value(selected)
        .map_err(|_| Refusal::InvalidInput)?;
    for step in steps {
        value = match step {
            Member::Field(name) => value.record_field(name),
            Member::Index(index) => value.collection_index(*index),
            Member::Variant(name) => value.variant_payload(name),
        }
        .map_err(|_| Refusal::InvalidInput)?
        .ok_or(Refusal::InvalidInput)?;
    }
    Ok(value)
}

pub(super) fn render_primitive<'a>(
    value: conduit_core::ValidatedCanonicalStructuredValue<'a>,
    output_type: &[u8],
) -> Result<&'a [u8], Refusal> {
    if value.type_bytes() != output_type {
        return Err(Refusal::InvalidInput);
    }
    let node = value.value_node();
    let [0, length @ ..] = node else {
        return Err(Refusal::InvalidProgram);
    };
    let length_bytes = length.get(..4).ok_or(Refusal::InvalidProgram)?;
    let size = u32::from_le_bytes(length_bytes.try_into().unwrap()) as usize;
    let bytes = length.get(4..).ok_or(Refusal::InvalidProgram)?;
    if bytes.len() != size {
        return Err(Refusal::InvalidProgram);
    }
    Ok(bytes)
}

pub(super) fn append_selected(
    value: conduit_core::ValidatedCanonicalStructuredValue<'_>,
    output_type: &[u8],
    output: &mut Vec<u8>,
) -> Result<(), Refusal> {
    if value.type_bytes() != output_type {
        return Err(Refusal::InvalidInput);
    }
    let node = value.value_node();
    if output
        .len()
        .checked_add(output_type.len())
        .and_then(|size| size.checked_add(node.len()))
        .is_none_or(|size| size > output.capacity())
    {
        return Err(Refusal::InvalidProgram);
    }
    output.extend_from_slice(output_type);
    output.extend_from_slice(node);
    Ok(())
}

pub(super) fn is_primitive(ty: &StructuredInfoType) -> bool {
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(_) => true,
        StructuredInfoTypeShape::Nominal { representation, .. } => is_primitive(representation),
        _ => false,
    }
}
