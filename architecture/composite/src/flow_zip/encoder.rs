//! Exact prepared pair representation, selected before execution.
use crate::prelude::*;
use conduit_core::{
    PreparedTuplePairEncoder, PreparedTypedTuplePairRuntimeEncoder, StructuredInfoRefusal,
};

pub(super) enum PairEncoder {
    Leaf(Box<PreparedTuplePairEncoder>),
    Typed(Box<PreparedTypedTuplePairRuntimeEncoder>),
    #[cfg(feature = "execution-workspace")]
    Workspace(Box<super::workspace_encoder::WorkspacePair>),
    #[cfg(feature = "execution-workspace")]
    StaticWorkspace(Box<super::static_workspace_encoder::StaticWorkspacePair>),
}
impl PairEncoder {
    #[cfg(feature = "schema-value-storage")]
    pub(super) fn input_schema(&self, side: usize) -> Option<&[u8]> {
        match self {
            Self::Typed(encoder) => encoder.canonical_input_schema(side),
            Self::Leaf(_) => None,
            #[cfg(feature = "execution-workspace")]
            Self::Workspace(encoder) => encoder.input_schema(side),
            #[cfg(feature = "execution-workspace")]
            Self::StaticWorkspace(encoder) => encoder.input_schema(side),
        }
    }
    #[cfg(feature = "schema-value-storage")]
    pub(super) fn output_schema(&self) -> Option<&[u8]> {
        match self {
            Self::Typed(encoder) => Some(encoder.canonical_output_schema()),
            Self::Leaf(_) => None,
            #[cfg(feature = "execution-workspace")]
            Self::Workspace(encoder) => Some(encoder.output_schema()),
            #[cfg(feature = "execution-workspace")]
            Self::StaticWorkspace(encoder) => Some(encoder.output_schema()),
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
            #[cfg(feature = "execution-workspace")]
            Self::Workspace(_) | Self::StaticWorkspace(_) => Err(StructuredInfoRefusal::WrongType),
        }
    }
    pub(super) fn encoded(&self) -> &[u8] {
        match self {
            Self::Leaf(encoder) => encoder.encoded(),
            Self::Typed(encoder) => encoder.encoded(),
            #[cfg(feature = "execution-workspace")]
            Self::Workspace(_) | Self::StaticWorkspace(_) => &[],
        }
    }
}

#[cfg(feature = "execution-workspace")]
impl PairEncoder {
    pub(super) fn workspace_maximum(&self) -> Option<u32> {
        match self {
            Self::Workspace(encoder) => Some(encoder.maximum()),
            Self::StaticWorkspace(encoder) => Some(encoder.maximum()),
            _ => None,
        }
    }
    pub(super) fn encode_into<'out>(
        &self,
        left: &[u8],
        right: &[u8],
        output: &'out mut [u8],
    ) -> Result<&'out [u8], StructuredInfoRefusal> {
        match self {
            Self::Workspace(encoder) => encoder.encode_into([left, right], output),
            Self::StaticWorkspace(encoder) => encoder.encode_into([left, right], output),
            _ => Err(StructuredInfoRefusal::WrongType),
        }
    }
}

#[cfg(feature = "schema-value-storage")]
impl PairEncoder {
    pub(super) fn validate_node(
        &self,
        side: usize,
        node: &[u8],
        maximum: usize,
    ) -> Result<(), StructuredInfoRefusal> {
        #[cfg(feature = "execution-workspace")]
        if let Self::StaticWorkspace(encoder) = self {
            return encoder.validate_node(side, node);
        }
        let schema = self
            .input_schema(side)
            .ok_or(StructuredInfoRefusal::WrongType)?;
        conduit_core::BorrowedStructuredValueValidator::new(schema, maximum)?
            .validate_node(node)
            .map(|_| ())
    }
}
