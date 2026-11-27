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
        let mut name =
            String::with_capacity("structured-info/profile-".len() + encoded.len() + "@1".len());
        name.push_str("structured-info/profile-");
        name.push_str(&encoded);
        name.push_str("@1");
        Ok(Self {
            value_type,
            value_kind: KindId::from(name),
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
