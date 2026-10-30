//! Complete validated structured configuration with owned or immutable linked bytes.
use crate::{KindId, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use alloc::{borrow::Cow, vec::Vec};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructuredConfigurationValue {
    profile: KindId,
    canonical_value: Cow<'static, [u8]>,
}

impl StructuredConfigurationValue {
    pub fn new(profile: KindId, canonical_value: Vec<u8>) -> Option<Self> {
        Self::from_storage(profile, Cow::Owned(canonical_value))
    }

    /// Retain complete immutable canonical bytes linked into this executable.
    /// Validation is identical to owned input; linkage conveys no checked
    /// Source provenance, capability custody or execution authority.
    pub fn from_static(profile: KindId, canonical_value: &'static [u8]) -> Option<Self> {
        Self::from_storage(profile, Cow::Borrowed(canonical_value))
    }

    fn from_storage(profile: KindId, canonical_value: Cow<'static, [u8]>) -> Option<Self> {
        if profile.as_str().is_empty()
            || canonical_value.is_empty()
            || canonical_value.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return None;
        }
        let actual_profile =
            crate::structured_info::validation::configuration_profile(&canonical_value).ok()?;
        (actual_profile.value_kind() == &profile).then_some(Self {
            profile,
            canonical_value,
        })
    }

    pub fn profile(&self) -> &KindId {
        &self.profile
    }

    pub fn canonical_value(&self) -> &[u8] {
        &self.canonical_value
    }
}
