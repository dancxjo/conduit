//! Finite, explicitly wired house context for an ordinary model request.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAXIMUM_HOUSE_CONTEXT_ITEMS: usize = 16;
pub const MAXIMUM_HOUSE_CONTEXT_BYTES: usize = 16_384;
pub const MAXIMUM_HOUSE_UTTERANCE_BYTES: usize = 4_096;
pub const MAXIMUM_HOUSE_CONTEXT_IDENTITY_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HouseContextProvenanceClass {
    /// An exact Sign identity accompanies the value; the model output does not inherit it.
    ObservedSign,
    /// Current body/profile configuration explicitly wired into this Form.
    DeclaredConfiguration,
    /// A prior model result, retained as model-derived rather than evidence.
    ModelDerivedHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WiredHouseContextItem {
    pub item_identity: String,
    pub value_kind: String,
    pub canonical_value: Vec<u8>,
    pub provenance: HouseContextProvenanceClass,
    pub source_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseModelRequest {
    pub request_identity: String,
    pub addressed_utterance: String,
    pub context: Vec<WiredHouseContextItem>,
    pub maximum_output_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HouseContextRefusal {
    EmptyUtterance,
    UtteranceBoundExceeded,
    EmptyContext,
    ContextItemLimitExceeded,
    ContextByteLimitExceeded,
    MissingIdentity,
    IdentityBoundExceeded,
    EmptyValue,
    DuplicateItem,
    InvalidOutputBound,
}

pub fn build_house_model_request(
    addressed_utterance: &str,
    explicitly_wired: &[WiredHouseContextItem],
    maximum_output_bytes: u64,
) -> Result<HouseModelRequest, HouseContextRefusal> {
    if addressed_utterance.is_empty() {
        return Err(HouseContextRefusal::EmptyUtterance);
    }
    if addressed_utterance.len() > MAXIMUM_HOUSE_UTTERANCE_BYTES {
        return Err(HouseContextRefusal::UtteranceBoundExceeded);
    }
    if explicitly_wired.is_empty() {
        return Err(HouseContextRefusal::EmptyContext);
    }
    if explicitly_wired.len() > MAXIMUM_HOUSE_CONTEXT_ITEMS {
        return Err(HouseContextRefusal::ContextItemLimitExceeded);
    }
    if maximum_output_bytes == 0 || maximum_output_bytes > crate::MAXIMUM_LLM_OUTPUT_BYTES {
        return Err(HouseContextRefusal::InvalidOutputBound);
    }

    let mut context_bytes = 0usize;
    for (index, item) in explicitly_wired.iter().enumerate() {
        let identities = [&item.item_identity, &item.value_kind, &item.source_identity];
        if identities.iter().any(|identity| identity.is_empty()) {
            return Err(HouseContextRefusal::MissingIdentity);
        }
        if identities
            .iter()
            .any(|identity| identity.len() > MAXIMUM_HOUSE_CONTEXT_IDENTITY_BYTES)
        {
            return Err(HouseContextRefusal::IdentityBoundExceeded);
        }
        if item.canonical_value.is_empty() {
            return Err(HouseContextRefusal::EmptyValue);
        }
        if explicitly_wired[index + 1..]
            .iter()
            .any(|other| other.item_identity == item.item_identity)
        {
            return Err(HouseContextRefusal::DuplicateItem);
        }
        context_bytes = context_bytes
            .checked_add(item.canonical_value.len())
            .ok_or(HouseContextRefusal::ContextByteLimitExceeded)?;
        if context_bytes > MAXIMUM_HOUSE_CONTEXT_BYTES {
            return Err(HouseContextRefusal::ContextByteLimitExceeded);
        }
    }

    let mut digest = Sha256::new();
    digest.update(b"conduit-house-model-request-v1\0");
    digest.update(addressed_utterance.as_bytes());
    digest.update(b"\0");
    digest.update(maximum_output_bytes.to_le_bytes());
    for item in explicitly_wired {
        digest.update(b"\0");
        digest.update(item.item_identity.as_bytes());
        digest.update(b"\0");
        digest.update(item.value_kind.as_bytes());
        digest.update(b"\0");
        digest.update(item.source_identity.as_bytes());
        digest.update(b"\0");
        digest.update([item.provenance as u8]);
        digest.update(b"\0");
        digest.update(&item.canonical_value);
    }

    Ok(HouseModelRequest {
        request_identity: alloc::format!("house-model-request/{:x}", digest.finalize()),
        addressed_utterance: addressed_utterance.into(),
        context: explicitly_wired.to_vec(),
        maximum_output_bytes,
    })
}
