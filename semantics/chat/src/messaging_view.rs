//! Provider-neutral validated views over portable delivery requests.

use alloc::{string::String, string::ToString, vec::Vec};
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};

use crate::messaging_reference::{leaf_text, record_field, validate_request};
use crate::MessagingInfoRefusal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessagingRecipientView {
    pub address: String,
    pub address_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessagingDeliveryRequestView {
    pub request_identity: String,
    pub correlation_identity: String,
    pub authority_identity: Option<String>,
    pub attempt: u64,
    pub body: String,
    pub recipients: Vec<MessagingRecipientView>,
    pub attachment_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessagingDeliveryStateView {
    pub state: String,
    pub evidence_kind: Option<String>,
    pub evidence_identity: Option<String>,
}

pub fn messaging_delivery_request_view(
    request: &StructuredInfoValue,
) -> Result<MessagingDeliveryRequestView, MessagingInfoRefusal> {
    validate_request(request)?;
    let message = record_field(request, "message")?;
    let authority = record_field(request, "authority")?;
    let authority_identity = match authority.shape() {
        StructuredInfoValueShape::Variant {
            tag: "grant",
            payload,
        } => Some(leaf_text(payload)?.to_string()),
        StructuredInfoValueShape::Variant { tag: "absent", .. } => None,
        _ => return Err(MessagingInfoRefusal::MalformedInfo),
    };
    let recipients = active_records(record_field(message, "recipients")?, "recipient")?
        .into_iter()
        .map(|recipient| {
            Ok(MessagingRecipientView {
                address: leaf_text(record_field(recipient, "address")?)?.to_string(),
                address_profile: leaf_text(record_field(recipient, "address_profile")?)?
                    .to_string(),
            })
        })
        .collect::<Result<Vec<_>, MessagingInfoRefusal>>()?;
    let attachment_count =
        u8::try_from(active_records(record_field(message, "attachments")?, "attachment")?.len())
            .map_err(|_| MessagingInfoRefusal::MalformedInfo)?;
    Ok(MessagingDeliveryRequestView {
        request_identity: leaf_text(record_field(request, "request_identity")?)?.to_string(),
        correlation_identity: leaf_text(record_field(request, "correlation_identity")?)?
            .to_string(),
        authority_identity,
        attempt: leaf_text(record_field(request, "attempt")?)?
            .parse()
            .map_err(|_| MessagingInfoRefusal::MalformedInfo)?,
        body: leaf_text(record_field(message, "body")?)?.to_string(),
        recipients,
        attachment_count,
    })
}

pub fn messaging_delivery_state_view(
    update: &StructuredInfoValue,
) -> Result<MessagingDeliveryStateView, MessagingInfoRefusal> {
    if update.value_type() != &crate::delivery_update_type() {
        return Err(MessagingInfoRefusal::MalformedInfo);
    }
    let state = record_field(update, "state")?;
    let StructuredInfoValueShape::Variant { tag, payload } = state.shape() else {
        return Err(MessagingInfoRefusal::MalformedInfo);
    };
    let (evidence_kind, evidence_identity) = match payload.shape() {
        StructuredInfoValueShape::Variant {
            tag: evidence_kind,
            payload: evidence_identity,
        } => (
            Some(evidence_kind.to_string()),
            Some(leaf_text(evidence_identity)?.to_string()),
        ),
        _ => (None, None),
    };
    Ok(MessagingDeliveryStateView {
        state: tag.to_string(),
        evidence_kind,
        evidence_identity,
    })
}

fn active_records<'a>(
    value: &'a StructuredInfoValue,
    active_tag: &str,
) -> Result<Vec<&'a StructuredInfoValue>, MessagingInfoRefusal> {
    let StructuredInfoValueShape::Collection(slots) = value.shape() else {
        return Err(MessagingInfoRefusal::MalformedInfo);
    };
    slots
        .iter()
        .filter_map(|slot| match slot.shape() {
            StructuredInfoValueShape::Variant { tag: "unused", .. } => None,
            StructuredInfoValueShape::Variant { tag, payload } if tag == active_tag => {
                Some(Ok(payload))
            }
            _ => Some(Err(MessagingInfoRefusal::MalformedInfo)),
        })
        .collect()
}
