//! Bounded provider-neutral text-message fixture construction.

use alloc::{vec, vec::Vec};
use conduit_core::{kind_id, StructuredFieldValue, StructuredInfoType, StructuredInfoValue};

use crate::{
    deterministic_delivery_request, message_attachment_slot_type, message_attachments_type,
    message_metadata_slot_type, message_metadata_type, message_recipient_slot_type,
    message_recipient_type, message_recipients_type, messaging_optional_text_type,
    portable_message_type, MessagingFixture, MessagingInfoRefusal, MAXIMUM_MESSAGE_ATTACHMENTS,
    MAXIMUM_MESSAGE_METADATA, MAXIMUM_MESSAGE_RECIPIENTS,
};

pub struct TextMessagingFixtureSpec<'a> {
    pub message_identity: &'a str,
    pub request_identity: &'a str,
    pub correlation_identity: &'a str,
    pub authority_identity: &'a str,
    pub recipient_address: &'a str,
    pub recipient_address_profile: &'a str,
    pub body: &'a str,
}

pub fn text_messaging_fixture(
    spec: TextMessagingFixtureSpec<'_>,
) -> Result<MessagingFixture, MessagingInfoRefusal> {
    let recipient = record(
        message_recipient_type(),
        vec![
            ("address", text(spec.recipient_address)),
            ("address_profile", text(spec.recipient_address_profile)),
            (
                "display_name",
                StructuredInfoValue::variant(
                    messaging_optional_text_type("messaging/optional-display-name@1"),
                    "absent",
                    unit()?,
                )?,
            ),
        ],
    )?;
    let recipients = slots(
        message_recipients_type(),
        message_recipient_slot_type(),
        Some(("recipient", recipient)),
        usize::from(MAXIMUM_MESSAGE_RECIPIENTS),
    )?;
    let attachments = slots(
        message_attachments_type(),
        message_attachment_slot_type(),
        None,
        usize::from(MAXIMUM_MESSAGE_ATTACHMENTS),
    )?;
    let metadata = slots(
        message_metadata_type(),
        message_metadata_slot_type(),
        None,
        usize::from(MAXIMUM_MESSAGE_METADATA),
    )?;
    let message = record(
        portable_message_type(),
        vec![
            ("attachments", attachments),
            ("body", text(spec.body)),
            ("message_identity", text(spec.message_identity)),
            ("metadata", metadata),
            ("recipients", recipients),
            (
                "sender",
                StructuredInfoValue::variant(
                    messaging_optional_text_type("messaging/optional-sender@1"),
                    "absent",
                    unit()?,
                )?,
            ),
            (
                "subject",
                StructuredInfoValue::variant(
                    messaging_optional_text_type("messaging/optional-subject@1"),
                    "absent",
                    unit()?,
                )?,
            ),
        ],
    )?;
    let request = deterministic_delivery_request(
        &message,
        1,
        Some(spec.authority_identity),
        spec.correlation_identity,
        spec.request_identity,
    )?;
    Ok(MessagingFixture { message, request })
}

fn slots(
    collection_type: StructuredInfoType,
    slot_type: StructuredInfoType,
    active: Option<(&str, StructuredInfoValue)>,
    length: usize,
) -> Result<StructuredInfoValue, MessagingInfoRefusal> {
    let mut values = Vec::with_capacity(length);
    if let Some((tag, value)) = active {
        values.push(StructuredInfoValue::variant(slot_type.clone(), tag, value)?);
    }
    while values.len() < length {
        values.push(StructuredInfoValue::variant(
            slot_type.clone(),
            "unused",
            unit()?,
        )?);
    }
    Ok(StructuredInfoValue::collection(collection_type, values)?)
}

fn record(
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

fn text(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/text@1")).unwrap(),
        value.as_bytes().to_vec(),
    )
    .expect("bounded messaging fixture text")
}

fn unit() -> Result<StructuredInfoValue, MessagingInfoRefusal> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/unit@1"))?,
        Vec::new(),
    )?)
}
