//! Runtime finite collection indexing over admitted canonical values.
use super::{
    member_selection, PreparedInput, PreparedPortableExpressionEvaluator, ProgramView, Refusal,
};
use crate::{PortableExpressionNode, PortableExpressionOperation};
use alloc::{boxed::Box, vec::Vec};
use conduit_core::{kind_id, StructuredInfoType};

pub(super) struct PreparedSequenceSelection {
    source: Box<PreparedPortableExpressionEvaluator>,
    index: Box<PreparedPortableExpressionEvaluator>,
    output_type: Vec<u8>,
    primitive: bool,
}
impl PreparedSequenceSelection {
    pub(super) fn new(
        node: &PortableExpressionNode,
        input: &StructuredInfoType,
        prepared_input: &PreparedInput,
    ) -> Result<Self, Refusal> {
        let PortableExpressionOperation::SemanticCall { kind, arguments } = &node.operation else {
            return Err(Refusal::InvalidProgram);
        };
        let [source, index] = arguments.as_slice() else {
            return Err(Refusal::InvalidProgram);
        };
        let element = crate::expression_semantic_call::collection_element(&source.value_type)
            .ok_or(Refusal::InvalidProgram)?;
        if kind != "sequence/at"
            || element != &node.value_type
            || index.value_type
                != StructuredInfoType::leaf(kind_id("value/u64"))
                    .map_err(|_| Refusal::InvalidProgram)?
        {
            return Err(Refusal::InvalidProgram);
        }
        let prepare = |child: &PortableExpressionNode| {
            PreparedPortableExpressionEvaluator::prepare(
                ProgramView {
                    input_maximum_bytes: prepared_input.maximum_bytes(),
                    input_type: input,
                    output_type: &child.value_type,
                    root: child,
                },
                prepared_input.clone(),
            )
        };
        let primitive = member_selection::is_primitive(element);
        Ok(Self {
            source: Box::new(prepare(source)?),
            index: Box::new(prepare(index)?),
            output_type: element
                .canonical_bytes()
                .map_err(|_| Refusal::InvalidProgram)?,
            primitive,
        })
    }
    pub(super) fn evaluate<'a>(&'a mut self, input: &'a [u8]) -> Result<&'a [u8], Refusal> {
        if !self.primitive {
            return Err(Refusal::InvalidProgram);
        }
        let value = select(&mut self.source, &mut self.index, input)?;
        member_selection::render_primitive(value, &self.output_type)
    }
    pub(super) fn evaluate_into(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<(), Refusal> {
        if self.primitive {
            return super::structured::append(output, self.evaluate(input)?);
        }
        let value = select(&mut self.source, &mut self.index, input)?;
        member_selection::append_selected(value, &self.output_type, output)
    }
}
fn select<'a>(
    source: &'a mut PreparedPortableExpressionEvaluator,
    index: &mut PreparedPortableExpressionEvaluator,
    input: &'a [u8],
) -> Result<conduit_core::ValidatedCanonicalStructuredValue<'a>, Refusal> {
    let index = index.evaluate(input)?;
    let index = u64::from_le_bytes(index.try_into().map_err(|_| Refusal::InvalidInput)?);
    let index = u16::try_from(index).map_err(|_| Refusal::InvalidInput)?;
    let source = source.evaluate(input)?;
    let source = conduit_core::validate_canonical_structured_value(source)
        .map_err(|_| Refusal::InvalidInput)?;
    source
        .collection_index(index)
        .map_err(|_| Refusal::InvalidInput)?
        .ok_or(Refusal::InvalidInput)
}
