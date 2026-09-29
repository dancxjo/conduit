use crate::prelude::*;
use conduit_core::{StructuredInfoRefusal, StructuredInfoType, StructuredInfoValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeBindingRefusal {
    InvalidSemanticType(StructuredInfoRefusal),
    InvalidValue(StructuredInfoRefusal),
}

/// Exact conversion boundary implemented by generated Rust bindings.
///
/// Implementations must recover their semantic Type from generated canonical
/// bytes. Rust type names and memory layout therefore cannot become identity.
pub trait NativeRustBinding: Sized {
    fn semantic_type() -> Result<StructuredInfoType, NativeBindingRefusal>;

    fn into_structured(self) -> Result<StructuredInfoValue, NativeBindingRefusal>;

    fn from_structured(value: StructuredInfoValue) -> Result<Self, NativeBindingRefusal>;

    fn encode(self) -> Result<Vec<u8>, NativeBindingRefusal> {
        self.into_structured()?
            .canonical_bytes()
            .map_err(NativeBindingRefusal::InvalidValue)
    }

    fn decode(canonical: &[u8]) -> Result<Self, NativeBindingRefusal> {
        let value = StructuredInfoValue::from_canonical_bytes(canonical)
            .map_err(NativeBindingRefusal::InvalidValue)?;
        if value.value_type() != &Self::semantic_type()? {
            return Err(NativeBindingRefusal::InvalidValue(
                StructuredInfoRefusal::WrongType,
            ));
        }
        Self::from_structured(value)
    }
}
