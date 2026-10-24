//! Retain exact checked Type bytes, then validate borrowed canonical envelopes.
//! Leaf semantics remain with the owning codec, as in Core's prepared validator.
use alloc::borrow::Cow;
use conduit_core::{
    StructuredInfoRefusal as R, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
pub(crate) struct PreparedCanonicalInput {
    prefix: Cow<'static, [u8]>,
    maximum_bytes: usize,
}
impl PreparedCanonicalInput {
    pub(crate) fn new(ty: &StructuredInfoType, maximum_bytes: usize) -> Result<Self, R> {
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(R::CanonicalEncodingTooLarge);
        }
        let prefix = ty.canonical_bytes()?;
        if prefix.len() >= maximum_bytes {
            return Err(R::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            prefix: Cow::Owned(prefix),
            maximum_bytes,
        })
    }
    pub(crate) fn with_static_schema(
        ty: &StructuredInfoType,
        prefix: &'static [u8],
        maximum_bytes: usize,
    ) -> Result<Self, R> {
        let mut prepared = Self::new(ty, maximum_bytes)?;
        if prepared.prefix.as_ref() != prefix {
            return Err(R::WrongType);
        }
        prepared.prefix = Cow::Borrowed(prefix);
        Ok(prepared)
    }
    pub(crate) fn validate(&self, input: &[u8]) -> Result<(), R> {
        if input.len() > self.maximum_bytes {
            return Err(R::CanonicalEncodingTooLarge);
        }
        if !input.starts_with(&self.prefix) {
            return Err(R::WrongType);
        }
        let validated = conduit_core::validate_canonical_structured_value(input)?;
        if validated.type_bytes() != self.prefix.as_ref() {
            return Err(R::WrongType);
        }
        Ok(())
    }
}
