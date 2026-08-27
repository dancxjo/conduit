//! Deterministic reference fixture and evidence-honest delivery lifecycle.

use alloc::{format, string::ToString, vec, vec::Vec};
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, StructuredFieldValue, StructuredInfoRefusal,
    StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape,
};

use crate::{
    delivery_evidence_type, delivery_request_type, delivery_state_type, delivery_update_type,
    message_attachment_slot_type, message_attachment_type, message_attachments_type,
    message_metadata_entry_type, message_metadata_slot_type, message_metadata_type,
    message_recipient_slot_type, message_recipient_type, message_recipients_type,
    messaging_optional_text_type, notification_event_type, portable_message_type,
    MAXIMUM_DELIVERY_ATTEMPTS, MAXIMUM_MESSAGE_ATTACHMENTS, MAXIMUM_MESSAGE_METADATA,
    MAXIMUM_MESSAGE_RECIPIENTS,
};

pub const MESSAGE_ATTACHMENT_PROFILE: &str = "messaging/attachment-content@1";
pub const MESSAGE_ATTACHMENT_ACCESS_CLASS: &str = "conduit.resource/message-attachment@1";

pub struct MessagingFixture {
    pub message: StructuredInfoValue,
    pub request: StructuredInfoValue,
}

#[derive(Debug)]
pub struct DeliveryResult {
    pub notification: StructuredInfoValue,
    pub update: StructuredInfoValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagingInfoRefusal {
    MalformedInfo,
    InvalidAttachment,
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for MessagingInfoRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

pub fn deterministic_messaging_fixture() -> Result<MessagingFixture, MessagingInfoRefusal> {
    let attachment = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([0x31; 32]),
        content_profile: KindId::from(MESSAGE_ATTACHMENT_PROFILE),
        access_class: ResourceClassId::from(MESSAGE_ATTACHMENT_ACCESS_CLASS),
        extent: ResourceExtent {
            bytes: 24,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([0x32; 32]),
            expires_at: None,
        },
    };
    attachment
        .validate()
        .map_err(|_| MessagingInfoRefusal::InvalidAttachment)?;
    let attachment_value = record_value(
        message_attachment_type(),
        vec![
            (
                "content",
                leaf_value(
                    conduit_core::RESOURCE_REFERENCE_INFO_ID,
                    attachment
                        .encode()
                        .map_err(|_| MessagingInfoRefusal::InvalidAttachment)?,
                )?,
            ),
            ("filename", text_value("lesson.txt")),
            ("media_type", text_value("text/plain")),
        ],
    )?;
    let attachment_slot = message_attachment_slot_type();
    let attachments = fixed_slots(
        message_attachments_type(),
        attachment_slot.clone(),
        "attachment",
        attachment_value,
        usize::from(MAXIMUM_MESSAGE_ATTACHMENTS),
    )?;

    let recipient = record_value(
        message_recipient_type(),
        vec![
            ("address", text_value("fixture/inbox")),
            ("address_profile", text_value("messaging/local-mailbox@1")),
            (
                "display_name",
                optional_text("messaging/optional-display-name@1", Some("Fixture Inbox"))?,
            ),
        ],
    )?;
    let recipient_slot = message_recipient_slot_type();
    let recipients = fixed_slots(
        message_recipients_type(),
        recipient_slot.clone(),
        "recipient",
        recipient,
        usize::from(MAXIMUM_MESSAGE_RECIPIENTS),
    )?;

    let metadata_entry = record_value(
        message_metadata_entry_type(),
        vec![
            ("key", text_value("lesson")),
            ("value", text_value("arithmetic")),
        ],
    )?;
    let metadata_slot = message_metadata_slot_type();
    let metadata = fixed_slots(
        message_metadata_type(),
        metadata_slot.clone(),
        "entry",
        metadata_entry,
        usize::from(MAXIMUM_MESSAGE_METADATA),
    )?;

    let message = record_value(
        portable_message_type(),
        vec![
            ("attachments", attachments),
            ("body", text_value("Your lesson result is ready.")),
            ("message_identity", text_value("message/fixture/1")),
            ("metadata", metadata),
            ("recipients", recipients),
            (
                "sender",
                optional_text("messaging/optional-sender@1", Some("lesson/service"))?,
            ),
            (
                "subject",
                optional_text("messaging/optional-subject@1", Some("Lesson result"))?,
            ),
        ],
    )?;
    let request = deterministic_delivery_request(
        &message,
        1,
        Some("authority/fixture-recipient/1"),
        "correlation/fixture/1",
        "delivery/fixture/1",
    )?;
    Ok(MessagingFixture { message, request })
}

pub fn deterministic_delivery_request(
    message: &StructuredInfoValue,
    attempt: u64,
    authority_identity: Option<&str>,
    correlation_identity: &str,
    request_identity: &str,
) -> Result<StructuredInfoValue, MessagingInfoRefusal> {
    if message.value_type() != &portable_message_type() {
        return Err(MessagingInfoRefusal::MalformedInfo);
    }
    let authority_type = crate::delivery_authority_type();
    let authority = match authority_identity {
        Some(identity) => {
            StructuredInfoValue::variant(authority_type, "grant", text_value(identity))?
        }
        None => StructuredInfoValue::variant(authority_type, "absent", unit_value()?)?,
    };
    record_value(
        delivery_request_type(),
        vec![
            ("attempt", count_value(attempt)),
            ("authority", authority),
            ("correlation_identity", text_value(correlation_identity)),
            ("message", message.clone()),
            ("request_identity", text_value(request_identity)),
        ],
    )
}

pub fn deterministic_submit(
    request: &StructuredInfoValue,
    duplicate_correlation_seen: bool,
) -> Result<DeliveryResult, MessagingInfoRefusal> {
    validate_request(request)?;
    let request_identity = leaf_text(record_field(request, "request_identity")?)?;
    let authority = variant_tag(record_field(request, "authority")?)?;
    let attempt = leaf_text(record_field(request, "attempt")?)?
        .parse::<u64>()
        .map_err(|_| MessagingInfoRefusal::MalformedInfo)?;
    let (tag, payload, summary) = if authority != "grant" {
        (
            "refused",
            text_value("recipient-authority-required"),
            "Delivery refused: recipient authority is absent.",
        )
    } else if duplicate_correlation_seen {
        (
            "duplicate",
            record_field(request, "correlation_identity")?.clone(),
            "Duplicate correlation refused.",
        )
    } else if attempt > MAXIMUM_DELIVERY_ATTEMPTS {
        (
            "failed",
            text_value("retry-limit-reached"),
            "Delivery failed: retry limit reached.",
        )
    } else {
        (
            "queued",
            evidence_value("local_queue", "fixture/local-queue/1")?,
            "Delivery admitted to the deterministic local queue.",
        )
    };
    delivery_result(request_identity, tag, payload, summary)
}

pub fn deterministic_provider_acknowledgement(
    request: &StructuredInfoValue,
) -> Result<DeliveryResult, MessagingInfoRefusal> {
    provider_acknowledgement(request, "fixture/provider-ack/1")
}

pub fn provider_acknowledgement(
    request: &StructuredInfoValue,
    evidence_identity: &str,
) -> Result<DeliveryResult, MessagingInfoRefusal> {
    validate_request(request)?;
    delivery_result(
        leaf_text(record_field(request, "request_identity")?)?,
        "sent",
        evidence_value("provider_acknowledgement", evidence_identity)?,
        "Provider acknowledged the request; recipient delivery is not claimed.",
    )
}

pub fn deterministic_cancel(
    request: &StructuredInfoValue,
) -> Result<DeliveryResult, MessagingInfoRefusal> {
    validate_request(request)?;
    delivery_result(
        leaf_text(record_field(request, "request_identity")?)?,
        "cancelled",
        text_value("caller-cancelled"),
        "Delivery was cancelled before recipient evidence.",
    )
}

pub(crate) fn validate_request(request: &StructuredInfoValue) -> Result<(), MessagingInfoRefusal> {
    if request.value_type() != &delivery_request_type() {
        return Err(MessagingInfoRefusal::MalformedInfo);
    }
    let message = record_field(request, "message")?;
    if message.value_type() != &portable_message_type() {
        return Err(MessagingInfoRefusal::MalformedInfo);
    }
    let StructuredInfoValueShape::Collection(slots) = record_field(message, "attachments")?.shape()
    else {
        return Err(MessagingInfoRefusal::MalformedInfo);
    };
    for slot in slots {
        let StructuredInfoValueShape::Variant { tag, payload } = slot.shape() else {
            return Err(MessagingInfoRefusal::MalformedInfo);
        };
        if tag == "unused" {
            continue;
        }
        if tag != "attachment" {
            return Err(MessagingInfoRefusal::MalformedInfo);
        }
        let reference = BoundedResourceRef::decode(leaf_bytes(record_field(payload, "content")?)?)
            .map_err(|_| MessagingInfoRefusal::InvalidAttachment)?;
        if reference.content_profile.as_str() != MESSAGE_ATTACHMENT_PROFILE
            || reference.access_class.as_str() != MESSAGE_ATTACHMENT_ACCESS_CLASS
        {
            return Err(MessagingInfoRefusal::InvalidAttachment);
        }
    }
    Ok(())
}

fn delivery_result(
    request_identity: &str,
    state_tag: &str,
    state_payload: StructuredInfoValue,
    summary: &str,
) -> Result<DeliveryResult, MessagingInfoRefusal> {
    let state = StructuredInfoValue::variant(delivery_state_type(), state_tag, state_payload)?;
    let update = record_value(
        delivery_update_type(),
        vec![
            ("request_identity", text_value(request_identity)),
            ("state", state),
        ],
    )?;
    let notification = record_value(
        notification_event_type(),
        vec![
            (
                "notification_identity",
                text_value(&format!("notification/{request_identity}")),
            ),
            ("source_request_identity", text_value(request_identity)),
            ("summary", text_value(summary)),
        ],
    )?;
    Ok(DeliveryResult {
        notification,
        update,
    })
}

fn evidence_value(tag: &str, identity: &str) -> Result<StructuredInfoValue, MessagingInfoRefusal> {
    Ok(StructuredInfoValue::variant(
        delivery_evidence_type(),
        tag,
        text_value(identity),
    )?)
}

fn fixed_slots(
    collection_type: StructuredInfoType,
    slot_type: StructuredInfoType,
    active_tag: &str,
    active: StructuredInfoValue,
    length: usize,
) -> Result<StructuredInfoValue, MessagingInfoRefusal> {
    let mut slots = vec![StructuredInfoValue::variant(
        slot_type.clone(),
        active_tag,
        active,
    )?];
    while slots.len() < length {
        slots.push(StructuredInfoValue::variant(
            slot_type.clone(),
            "unused",
            unit_value()?,
        )?);
    }
    Ok(StructuredInfoValue::collection(collection_type, slots)?)
}

fn optional_text(
    kind: &str,
    value: Option<&str>,
) -> Result<StructuredInfoValue, MessagingInfoRefusal> {
    let value_type = messaging_optional_text_type(kind);
    match value {
        Some(value) => Ok(StructuredInfoValue::variant(
            value_type,
            "provided",
            text_value(value),
        )?),
        None => Ok(StructuredInfoValue::variant(
            value_type,
            "absent",
            unit_value()?,
        )?),
    }
}

fn unit_value() -> Result<StructuredInfoValue, MessagingInfoRefusal> {
    leaf_value("value/unit@1", Vec::new())
}

fn text_value(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/text@1")).unwrap(),
        value.as_bytes().to_vec(),
    )
    .expect("bounded deterministic messaging text")
}

fn count_value(value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/count@1")).unwrap(),
        value.to_string().into_bytes(),
    )
    .expect("bounded deterministic messaging count")
}

fn leaf_value(kind: &str, bytes: Vec<u8>) -> Result<StructuredInfoValue, MessagingInfoRefusal> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(kind))?,
        bytes,
    )?)
}

fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, MessagingInfoRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}

pub(crate) fn record_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, MessagingInfoRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(MessagingInfoRefusal::MalformedInfo);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(MessagingInfoRefusal::MalformedInfo)
}

fn variant_tag(value: &StructuredInfoValue) -> Result<&str, MessagingInfoRefusal> {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        return Err(MessagingInfoRefusal::MalformedInfo);
    };
    Ok(tag)
}

pub(crate) fn leaf_text(value: &StructuredInfoValue) -> Result<&str, MessagingInfoRefusal> {
    core::str::from_utf8(leaf_bytes(value)?).map_err(|_| MessagingInfoRefusal::MalformedInfo)
}

fn leaf_bytes(value: &StructuredInfoValue) -> Result<&[u8], MessagingInfoRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(MessagingInfoRefusal::MalformedInfo);
    };
    Ok(bytes)
}
