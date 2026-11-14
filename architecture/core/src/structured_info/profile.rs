use alloc::format;
use alloc::string::String;

use crate::KindId;

use super::{StructuredInfoRefusal, StructuredInfoType};

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Exact portable Port profile derived from structured semantic shape.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StructuredInfoProfile {
    value_type: StructuredInfoType,
    value_kind: KindId,
}

impl StructuredInfoProfile {
    pub fn new(value_type: StructuredInfoType) -> Result<Self, StructuredInfoRefusal> {
        let digest = value_type.semantic_digest()?;
        let mut encoded = String::with_capacity(digest.len() * 2);
        for byte in digest {
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        Ok(Self {
            value_type,
            value_kind: KindId::from(format!("structured-info/profile-{encoded}@1")),
        })
    }

    pub fn value_type(&self) -> &StructuredInfoType {
        &self.value_type
    }

    pub fn value_kind(&self) -> &KindId {
        &self.value_kind
    }
}

impl StructuredInfoType {
    pub fn profile(&self) -> Result<StructuredInfoProfile, StructuredInfoRefusal> {
        StructuredInfoProfile::new(self.clone())
    }
}

/// Compare the existing exact profile identity without constructing an owned
/// Type tree or allocating a formatted KindId. Caller supplies validated schema.
pub(super) fn matches_canonical_schema(schema: &[u8], identity: &str) -> bool {
    let Some(hex) = identity
        .strip_prefix("structured-info/profile-")
        .and_then(|text| text.strip_suffix("@1"))
    else {
        return false;
    };
    let Ok(digest) = super::canonical::digest(super::TYPE_DIGEST_DOMAIN, schema) else {
        return false;
    };
    hex.len() == 64
        && digest.iter().enumerate().all(|(index, byte)| {
            hex.as_bytes()[2 * index] == HEX[usize::from(byte >> 4)]
                && hex.as_bytes()[2 * index + 1] == HEX[usize::from(byte & 15)]
        })
}
