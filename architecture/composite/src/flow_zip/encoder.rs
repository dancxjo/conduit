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
