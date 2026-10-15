//! Sealed canonical storage for concrete, already admitted startup defaults.
use crate::{prelude::*, CanonicalStructuredStartupValue};
use conduit_core::{StructuredInfoRefusal, StructuredInfoValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredStructuredDefault {
    canonical: Vec<u8>,
}

impl StoredStructuredDefault {
    pub(crate) fn from_checked_value(
        value: &StructuredInfoValue,
    ) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            canonical: value.canonical_bytes()?,
        })
    }

    pub(crate) fn resolve(&self) -> Result<CanonicalStructuredStartupValue, StructuredInfoRefusal> {
        let value = StructuredInfoValue::from_canonical_bytes(&self.canonical)?;
        Ok(CanonicalStructuredStartupValue::from_owned_checked_value(
            value,
        ))
    }
}
