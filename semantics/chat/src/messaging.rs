//! Native Conduitese-owned message, delivery, notification, and presence Info.
//!
//! Provider addresses, transports, acknowledgement guarantees, and retry
//! execution remain realization facts. Attachments remain bounded resources.

use alloc::{vec, vec::Vec};
use conduit_core::StructuredInfoType;

use crate::{
    DeliveryAuthority, DeliveryEvidence, DeliveryRequest, DeliveryState, DeliveryUpdate,
    MessageAttachment, MessageAttachmentSlot, MessageMetadataEntry, MessageMetadataSlot,
    MessageOptionalDisplayName, MessageOptionalSender, MessageOptionalSubject, MessageRecipient,
    MessageRecipientSlot, NotificationEvent, PortableMessage, PresenceEvent, PresenceState,
};

pub const PORTABLE_MESSAGE_TYPE: &str = "PortableMessage";
pub const DELIVERY_REQUEST_TYPE: &str = "DeliveryRequest";
pub const DELIVERY_UPDATE_TYPE: &str = "DeliveryUpdate";
pub const NOTIFICATION_EVENT_TYPE: &str = "NotificationEvent";
pub const PRESENCE_EVENT_TYPE: &str = "PresenceEvent";
pub const MAXIMUM_MESSAGE_RECIPIENTS: u16 = 4;
pub const MAXIMUM_MESSAGE_METADATA: u16 = 4;
pub const MAXIMUM_MESSAGE_ATTACHMENTS: u16 = 2;
pub const MAXIMUM_DELIVERY_ATTEMPTS: u64 = 3;

fn checked(
    value: Result<StructuredInfoType, conduit_plot::rust_binding::NativeBindingRefusal>,
) -> StructuredInfoType {
    value.expect("checked native messaging Type is finite")
}

pub fn message_optional_display_name_type() -> StructuredInfoType {
    checked(MessageOptionalDisplayName::semantic_type())
}

pub fn message_optional_sender_type() -> StructuredInfoType {
    checked(MessageOptionalSender::semantic_type())
}

pub fn message_optional_subject_type() -> StructuredInfoType {
    checked(MessageOptionalSubject::semantic_type())
}

pub fn message_recipient_type() -> StructuredInfoType {
    checked(MessageRecipient::semantic_type())
}

pub fn message_recipient_slot_type() -> StructuredInfoType {
    checked(MessageRecipientSlot::semantic_type())
}

pub fn message_recipients_type() -> StructuredInfoType {
    let record = checked(PortableMessage::semantic_type());
    conduit_plot::rust_binding::record_field_type(&record, "recipients")
        .expect("native recipient collection field")
}

pub fn message_metadata_entry_type() -> StructuredInfoType {
    checked(MessageMetadataEntry::semantic_type())
}

pub fn message_metadata_slot_type() -> StructuredInfoType {
    checked(MessageMetadataSlot::semantic_type())
}

pub fn message_metadata_type() -> StructuredInfoType {
    let record = checked(PortableMessage::semantic_type());
    conduit_plot::rust_binding::record_field_type(&record, "metadata")
        .expect("native metadata collection field")
}

pub fn message_attachment_type() -> StructuredInfoType {
    checked(MessageAttachment::semantic_type())
}

pub fn message_attachment_slot_type() -> StructuredInfoType {
    checked(MessageAttachmentSlot::semantic_type())
}

pub fn message_attachments_type() -> StructuredInfoType {
    let record = checked(PortableMessage::semantic_type());
    conduit_plot::rust_binding::record_field_type(&record, "attachments")
        .expect("native attachment collection field")
}

pub fn portable_message_type() -> StructuredInfoType {
    checked(PortableMessage::semantic_type())
}

pub fn delivery_authority_type() -> StructuredInfoType {
    checked(DeliveryAuthority::semantic_type())
}

pub fn delivery_request_type() -> StructuredInfoType {
    checked(DeliveryRequest::semantic_type())
}

pub fn delivery_evidence_type() -> StructuredInfoType {
    checked(DeliveryEvidence::semantic_type())
}

pub fn delivery_state_type() -> StructuredInfoType {
    checked(DeliveryState::semantic_type())
}

pub fn delivery_update_type() -> StructuredInfoType {
    checked(DeliveryUpdate::semantic_type())
}

pub fn notification_event_type() -> StructuredInfoType {
    checked(NotificationEvent::semantic_type())
}

pub fn presence_state_type() -> StructuredInfoType {
    checked(PresenceState::semantic_type())
}

pub fn presence_event_type() -> StructuredInfoType {
    checked(PresenceEvent::semantic_type())
}

pub fn messaging_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (
            "MessageOptionalDisplayName",
            message_optional_display_name_type(),
        ),
        ("MessageOptionalSender", message_optional_sender_type()),
        ("MessageOptionalSubject", message_optional_subject_type()),
        ("MessageRecipient", message_recipient_type()),
        ("MessageRecipientSlot", message_recipient_slot_type()),
        ("MessageRecipients", message_recipients_type()),
        ("MessageMetadataEntry", message_metadata_entry_type()),
        ("MessageMetadataSlot", message_metadata_slot_type()),
        ("MessageMetadata", message_metadata_type()),
        ("MessageAttachment", message_attachment_type()),
        ("MessageAttachmentSlot", message_attachment_slot_type()),
        ("MessageAttachments", message_attachments_type()),
        (PORTABLE_MESSAGE_TYPE, portable_message_type()),
        ("DeliveryAuthority", delivery_authority_type()),
        (DELIVERY_REQUEST_TYPE, delivery_request_type()),
        ("DeliveryEvidence", delivery_evidence_type()),
        ("DeliveryState", delivery_state_type()),
        (DELIVERY_UPDATE_TYPE, delivery_update_type()),
        (NOTIFICATION_EVENT_TYPE, notification_event_type()),
        (PRESENCE_EVENT_TYPE, presence_event_type()),
    ]
}
