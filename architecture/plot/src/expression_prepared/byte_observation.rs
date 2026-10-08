//! Observations of packed bytes retain their actual extent, without collection framing.
use super::{
    evaluate_node, prepare_node, primitive::PrimitiveValue, EvaluationInput, PreparedInput,
    PreparedNode, Refusal,
};
use crate::PortableExpressionNode;
use alloc::boxed::Box;
use conduit_core::{kind_id, PrimitiveInfoKind, StructuredInfoType};

pub(super) struct PreparedByteObservation {
    source: Box<PreparedNode>,
    index: Option<Box<PreparedNode>>,
}
impl PreparedByteObservation {
    pub(super) fn new(
        call: &str,
        arguments: &[PortableExpressionNode],
        input: &StructuredInfoType,
        prepared_input: &PreparedInput,
    ) -> Result<Self, Refusal> {
        let expected = if call == "bytes/at" {
            2
        } else if call == "bytes/length" {
            1
        } else {
            return Err(Refusal::InvalidProgram);
        };
        if arguments.len() != expected
            || arguments[0].value_type
                != StructuredInfoType::leaf(kind_id("value/bytes"))
                    .map_err(|_| Refusal::InvalidProgram)?
        {
            return Err(Refusal::InvalidProgram);
        }
        let prepare = |node: &PortableExpressionNode| prepare_node(node, input, prepared_input);
        let index = if call == "bytes/at" {
            let node = &arguments[1];
            if node.value_type
                != StructuredInfoType::leaf(kind_id("value/u64"))
                    .map_err(|_| Refusal::InvalidProgram)?
            {
                return Err(Refusal::InvalidProgram);
            }
            Some(Box::new(prepare(node)?))
        } else {
            None
        };
        Ok(Self {
            source: Box::new(prepare(&arguments[0])?),
            index,
        })
    }
    pub(super) fn evaluate(
        &mut self,
        input: &[u8],
        input_kind: Option<PrimitiveInfoKind>,
        canonical: EvaluationInput<'_>,
    ) -> Result<PrimitiveValue<'_>, Refusal> {
        let source = evaluate_node(&mut self.source, input, input_kind, canonical)?;
        let bytes = source.as_slice();
        if let Some(index) = &mut self.index {
            let index_value = evaluate_node(index, input, input_kind, canonical)?;
            let encoded = index_value.as_slice();
            let index = u64::from_le_bytes(encoded.try_into().map_err(|_| Refusal::InvalidInput)?);
            let index = usize::try_from(index).map_err(|_| Refusal::InvalidInput)?;
            let octet = bytes.get(index).ok_or(Refusal::InvalidInput)?;
            PrimitiveValue::new(PrimitiveInfoKind::U8, core::slice::from_ref(octet))
        } else {
            PrimitiveValue::new(PrimitiveInfoKind::U64, &(bytes.len() as u64).to_le_bytes())
        }
    }
}

impl PreparedByteObservation {
    pub(super) fn owned_heap_bytes(&self) -> usize {
        super::storage::boxed(self.source.as_ref(), self.source.owned_heap_bytes()).saturating_add(
            self.index.as_ref().map_or(0, |index| {
                super::storage::boxed(index.as_ref(), index.owned_heap_bytes())
            }),
        )
    }
}
