//! Finite, explicitly wired house context for an ordinary model request.

use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};
use sha2::{Digest, Sha256};

use crate::{HouseContextRefusal, HouseModelRequest, WiredHouseContextItem};

pub const MAXIMUM_HOUSE_CONTEXT_ITEMS: usize = 16;
pub const MAXIMUM_HOUSE_CONTEXT_BYTES: usize = 16_384;
pub const MAXIMUM_HOUSE_UTTERANCE_BYTES: usize = 4_096;
pub const MAXIMUM_HOUSE_CONTEXT_IDENTITY_BYTES: usize = 256;

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
        if item.canonical_value().as_slice().is_empty() {
            return Err(HouseContextRefusal::EmptyValue);
        }
        if explicitly_wired[index + 1..]
            .iter()
            .any(|other| other.item_identity() == item.item_identity())
        {
            return Err(HouseContextRefusal::DuplicateItem);
        }
        context_bytes = context_bytes
            .checked_add(item.canonical_value().as_slice().len())
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
        digest.update(item.item_identity().as_bytes());
        digest.update(b"\0");
        digest.update(item.value_kind().as_bytes());
        digest.update(b"\0");
        digest.update(item.source_identity().as_bytes());
        digest.update(b"\0");
        digest.update([*item.provenance() as u8]);
        digest.update(b"\0");
        digest.update(item.canonical_value().as_slice());
    }

    HouseModelRequest::new(
        alloc::format!("house-model-request/{:x}", digest.finalize()),
        addressed_utterance.into(),
        BoundedSequence::try_from_iter(explicitly_wired.iter().cloned())
            .expect("validated context fits its native bound"),
        maximum_output_bytes,
    )
    .map_err(|_| HouseContextRefusal::InvalidOutputBound)
}

pub fn wired_house_context_item(
    item_identity: &str,
    value_kind: &str,
    canonical_value: &[u8],
    provenance: crate::HouseContextProvenanceClass,
    source_identity: &str,
) -> Result<WiredHouseContextItem, HouseContextRefusal> {
    if canonical_value.len() > MAXIMUM_HOUSE_CONTEXT_BYTES {
        return Err(HouseContextRefusal::ContextByteLimitExceeded);
    }
    let identity = |value: &str| -> Result<alloc::string::String, HouseContextRefusal> {
        if value.is_empty() {
            Err(HouseContextRefusal::MissingIdentity)
        } else if value.len() > MAXIMUM_HOUSE_CONTEXT_IDENTITY_BYTES {
            Err(HouseContextRefusal::IdentityBoundExceeded)
        } else {
            Ok(value.into())
        }
    };
    WiredHouseContextItem::new(
        identity(item_identity)?,
        identity(value_kind)?,
        BoundedBytes::new(canonical_value).ok_or(HouseContextRefusal::ContextByteLimitExceeded)?,
        provenance,
        identity(source_identity)?,
    )
    .map_err(|_| HouseContextRefusal::IdentityBoundExceeded)
}
