//! Runtime finite collection indexing over admitted canonical values.
use super::{
    member_selection, storage_bound, EvaluationInput, PreparedInput,
    PreparedPortableExpressionEvaluator, ProgramView, Refusal,
};
use crate::{PortableExpressionNode, PortableExpressionOperation};
use alloc::{boxed::Box, vec::Vec};
use conduit_core::{kind_id, StructuredInfoType, StructuredInfoTypeShape};

pub(super) struct PreparedSequenceSelection {
    source: Box<PreparedPortableExpressionEvaluator>,
    index: Box<PreparedPortableExpressionEvaluator>,
    output_type: Vec<u8>,
    output: Vec<u8>,
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
        let element = match source.value_type.shape() {
            StructuredInfoTypeShape::Collection { element, .. }
            | StructuredInfoTypeShape::Sequence { element, .. } => element,
            _ => return Err(Refusal::InvalidProgram),
        };
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
            output: Vec::with_capacity(if primitive {
                0
            } else {
                storage_bound::canonical(element)?
            }),
            primitive,
        })
    }
    pub(super) fn evaluate<'a>(
        &'a mut self,
        input: EvaluationInput<'a>,
    ) -> Result<&'a [u8], Refusal> {
        let index = self.index.evaluate_input(input)?;
        let index = u64::from_le_bytes(index.try_into().map_err(|_| Refusal::InvalidInput)?);
        let index = u16::try_from(index).map_err(|_| Refusal::InvalidInput)?;
        let source = self.source.evaluate_input(input)?;
        let source = conduit_core::validate_canonical_structured_value(source)
            .map_err(|_| Refusal::InvalidInput)?;
        let value = source
            .collection_index(index)
            .map_err(|_| Refusal::InvalidInput)?
            .ok_or(Refusal::InvalidInput)?;
        member_selection::render_selected(
            value,
            &self.output_type,
            self.primitive,
            &mut self.output,
        )
    }
}

impl PreparedSequenceSelection {
    pub(super) fn owned_heap_bytes(&self) -> usize {
        super::storage::boxed(self.source.as_ref(), self.source.owned_heap_bytes())
            .saturating_add(super::storage::boxed(
                self.index.as_ref(),
                self.index.owned_heap_bytes(),
            ))
            .saturating_add(self.output_type.capacity())
            .saturating_add(self.output.capacity())
    }
}
