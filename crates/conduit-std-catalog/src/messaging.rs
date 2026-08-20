//! Portable finite message, delivery, notification, and presence Info.
//!
//! Provider addresses, transports, acknowledgement guarantees, and retry
//! execution remain realization facts. Attachments remain bounded resources.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredInfoType, StructuredVariantCase,
    RESOURCE_REFERENCE_INFO_ID,
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

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed messaging leaf")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text@1")
}

fn count_type() -> StructuredInfoType {
    leaf("value/count@1")
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit@1")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed messaging field")
}

fn case(name: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload_type).expect("reviewed messaging case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed messaging record")
}

pub fn messaging_optional_text_type(kind: &str) -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id(kind),
        vec![case("absent", unit_type()), case("provided", text_type())],
    )
    .expect("reviewed optional messaging text")
}

pub fn message_recipient_type() -> StructuredInfoType {
    record(
        "messaging/recipient@1",
        vec![
            field("address", text_type()),
            field("address_profile", text_type()),
            field(
                "display_name",
                messaging_optional_text_type("messaging/optional-display-name@1"),
            ),
        ],
    )
}

pub fn message_recipient_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("messaging/recipient-slot@1"),
        vec![
            case("recipient", message_recipient_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed recipient slot")
}

pub fn message_recipients_type() -> StructuredInfoType {
    StructuredInfoType::collection(
        message_recipient_slot_type(),
        Some(MAXIMUM_MESSAGE_RECIPIENTS),
    )
    .expect("fixed recipient slots")
}

pub fn message_metadata_entry_type() -> StructuredInfoType {
    record(
        "messaging/metadata-entry@1",
        vec![field("key", text_type()), field("value", text_type())],
    )
}

pub fn message_metadata_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("messaging/metadata-slot@1"),
        vec![
            case("entry", message_metadata_entry_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed metadata slot")
}

pub fn message_metadata_type() -> StructuredInfoType {
    StructuredInfoType::collection(message_metadata_slot_type(), Some(MAXIMUM_MESSAGE_METADATA))
        .expect("fixed metadata slots")
}

pub fn message_attachment_type() -> StructuredInfoType {
    record(
        "messaging/attachment@1",
        vec![
            field("content", leaf(RESOURCE_REFERENCE_INFO_ID)),
            field("filename", text_type()),
            field("media_type", text_type()),
        ],
    )
}

pub fn message_attachment_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("messaging/attachment-slot@1"),
        vec![
            case("attachment", message_attachment_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed attachment slot")
}

pub fn message_attachments_type() -> StructuredInfoType {
    StructuredInfoType::collection(
        message_attachment_slot_type(),
        Some(MAXIMUM_MESSAGE_ATTACHMENTS),
    )
    .expect("fixed attachment slots")
}

pub fn portable_message_type() -> StructuredInfoType {
    record(
        "messaging/message@2",
        vec![
            field("attachments", message_attachments_type()),
            field("body", text_type()),
            field("message_identity", text_type()),
            field("metadata", message_metadata_type()),
            field("recipients", message_recipients_type()),
            field(
                "sender",
                messaging_optional_text_type("messaging/optional-sender@1"),
            ),
            field(
                "subject",
                messaging_optional_text_type("messaging/optional-subject@1"),
            ),
        ],
    )
}

pub fn delivery_authority_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("messaging/delivery-authority@1"),
        vec![case("absent", unit_type()), case("grant", text_type())],
    )
    .expect("reviewed delivery authority")
}

pub fn delivery_request_type() -> StructuredInfoType {
    record(
        "messaging/delivery-request@1",
        vec![
            field("attempt", count_type()),
            field("authority", delivery_authority_type()),
            field("correlation_identity", text_type()),
            field("message", portable_message_type()),
            field("request_identity", text_type()),
        ],
    )
}

pub fn delivery_evidence_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("messaging/delivery-evidence@1"),
        vec![
            case("end_recipient", text_type()),
            case("local_queue", text_type()),
            case("provider_acknowledgement", text_type()),
        ],
    )
    .expect("reviewed delivery evidence")
}

pub fn delivery_state_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("messaging/delivery-state@2"),
        vec![
            case("cancelled", text_type()),
            case("delivered", delivery_evidence_type()),
            case("duplicate", text_type()),
            case("expired", text_type()),
            case("failed", text_type()),
            case("queued", delivery_evidence_type()),
            case("refused", text_type()),
            case("sent", delivery_evidence_type()),
        ],
    )
    .expect("reviewed delivery states")
}

pub fn delivery_update_type() -> StructuredInfoType {
    record(
        "messaging/delivery-update@1",
        vec![
            field("request_identity", text_type()),
            field("state", delivery_state_type()),
        ],
    )
}

pub fn notification_event_type() -> StructuredInfoType {
    record(
        "notification/event@1",
        vec![
            field("notification_identity", text_type()),
            field("source_request_identity", text_type()),
            field("summary", text_type()),
        ],
    )
}

pub fn presence_state_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("presence/state@1"),
        vec![
            case("available", unit_type()),
            case("away", unit_type()),
            case("offline", unit_type()),
            case("unknown", unit_type()),
        ],
    )
    .expect("reviewed presence states")
}

pub fn presence_event_type() -> StructuredInfoType {
    record(
        "presence/event@1",
        vec![
            field("state", presence_state_type()),
            field("subject_identity", text_type()),
        ],
    )
}

pub fn messaging_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (PORTABLE_MESSAGE_TYPE, portable_message_type()),
        (DELIVERY_REQUEST_TYPE, delivery_request_type()),
        (DELIVERY_UPDATE_TYPE, delivery_update_type()),
        (NOTIFICATION_EVENT_TYPE, notification_event_type()),
        (PRESENCE_EVENT_TYPE, presence_event_type()),
    ]
}
