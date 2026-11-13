//! Exact prepared pair representation, selected before execution.
use crate::prelude::*;
use conduit_core::{
    PreparedTuplePairEncoder, PreparedTypedTuplePairRuntimeEncoder, StructuredInfoRefusal,
};

pub(super) enum PairEncoder {
    Leaf(Box<PreparedTuplePairEncoder>),
    Typed(Box<PreparedTypedTuplePairRuntimeEncoder>),
}
impl PairEncoder {
    #[cfg(feature = "schema-value-storage")]
    pub(super) fn input_schema(&self, side: usize) -> Option<&[u8]> {
        match self {
            Self::Typed(encoder) => encoder.canonical_input_schema(side),
            Self::Leaf(_) => None,
        }
    }
    #[cfg(feature = "schema-value-storage")]
    pub(super) fn output_schema(&self) -> Option<&[u8]> {
        match self {
            Self::Typed(encoder) => Some(encoder.canonical_output_schema()),
            Self::Leaf(_) => None,
        }
    }
    pub(super) fn encode(
        &mut self,
        left: &[u8],
        right: &[u8],
    ) -> Result<&[u8], StructuredInfoRefusal> {
        match self {
            Self::Leaf(encoder) => encoder.encode(left, right),
            Self::Typed(encoder) => encoder.encode(left, right),
        }
    }
    pub(super) fn encoded(&self) -> &[u8] {
        match self {
            Self::Leaf(encoder) => encoder.encoded(),
            Self::Typed(encoder) => encoder.encoded(),
        }
    }
}
