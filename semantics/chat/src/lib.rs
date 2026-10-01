//! Generic, transport-neutral chat contracts and bounded conversation state.
//!
//! Speech recognition and synthesis compose around these contracts in Tongues;
//! they do not own Body prompt, history, or context semantics.
#![no_std]

extern crate alloc;

#[allow(dead_code, clippy::large_enum_variant, clippy::too_many_arguments)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    BodyChatHistoryItem, BodyChatMessage, BodyChatRefusal, BodyChatRole, BodyChatRoleCode,
    BodyConversationalSummary, ChatConnectionState, ChatStateRefusal, DeliveryAuthority,
    DeliveryEvidence, DeliveryRequest, DeliveryState, DeliveryUpdate, MessageAttachment,
    MessageAttachmentSlot, MessageMetadataEntry, MessageMetadataSlot, MessageOptionalDisplayName,
    MessageOptionalSender, MessageOptionalSubject, MessageRecipient, MessageRecipientSlot,
    NotificationEvent, PortableMessage, PresenceEvent, PresenceState,
};

mod body_chat;
pub use body_chat::*;
mod conversation_request_evidence;
pub use conversation_request_evidence::ConversationRequestEvidence;

mod shared_pool;
pub use shared_pool::*;
mod interactive_state;
pub use interactive_state::*;
mod interactive_catalog;
pub use interactive_catalog::*;
mod messaging;
pub use messaging::*;
mod messaging_fixture;
pub use messaging_fixture::*;
mod messaging_reference;
pub use messaging_reference::*;
mod messaging_view;
pub use messaging_view::*;
#[cfg(feature = "form-catalog")]
mod messaging_catalog;
#[cfg(feature = "form-catalog")]
pub use messaging_catalog::*;

#[cfg(test)]
mod native_type_tests {
    use super::{
        BodyChatHistoryItem, BodyChatMessage, BodyChatRefusal, BodyChatRole, ChatConnectionState,
        ChatStateRefusal, MessageAttachment, PresenceState,
    };
    use conduit_core::{
        BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
        ResourceSemanticIdentity, ResourceVersionIdentity,
    };
    use conduit_form::rust_binding::NativeRustBinding;

    fn assert_round_trip<T>(value: T)
    where
        T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
    {
        let structured = value.into_structured().expect("native value encodes");
        assert_eq!(
            structured.value_type(),
            &T::semantic_type().expect("native semantic type checks")
        );
        assert_eq!(
            T::from_structured(structured).expect("native value decodes"),
            value
        );
    }

    #[test]
    fn chat_connection_and_presence_are_native_semantic_types() {
        assert_round_trip(ChatConnectionState::Connected);
        assert_round_trip(PresenceState::Away);
    }

    #[test]
    fn chat_refusals_are_native_semantic_types() {
        for refusal in [
            ChatStateRefusal::InvalidConfiguration,
            ChatStateRefusal::EmptyMessage,
            ChatStateRefusal::OversizeMessage,
            ChatStateRefusal::MalformedMessage,
            ChatStateRefusal::SequenceExhausted,
            ChatStateRefusal::InvalidPresentation,
        ] {
            assert_round_trip(refusal);
        }
        for refusal in [
            BodyChatRefusal::EmptyMessage,
            BodyChatRefusal::MessageBoundExceeded,
            BodyChatRefusal::ContextBoundExceeded,
            BodyChatRefusal::MalformedContext,
            BodyChatRefusal::WrongContextSchema,
            BodyChatRefusal::HistoryBoundExceeded,
            BodyChatRefusal::PromptBoundExceeded,
            BodyChatRefusal::Encoding,
        ] {
            assert_round_trip(refusal);
        }
    }

    #[test]
    fn body_chat_history_is_one_bounded_native_record() {
        let message = BodyChatMessage::new("hello".into()).unwrap();
        let item = BodyChatHistoryItem::new(BodyChatRole::Human, message).unwrap();
        let structured = item.clone().into_structured().unwrap();
        assert_eq!(
            BodyChatHistoryItem::from_structured(structured).unwrap(),
            item
        );
        assert!(BodyChatMessage::new(alloc::string::String::new()).is_err());
        assert!(BodyChatMessage::new("x".repeat(4_097)).is_err());
    }

    #[test]
    fn message_attachments_keep_the_existing_leaf_bound_and_resource_contract() {
        let reference = BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: KindId::from("messaging/attachment-content@1"),
            access_class: ResourceClassId::from("conduit.resource/message-attachment@1"),
            extent: ResourceExtent {
                bytes: 4_096,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        };
        let attachment = MessageAttachment::new(
            reference,
            "f".repeat(conduit_core::MAXIMUM_STRUCTURED_LEAF_BYTES),
            "text/plain".into(),
        )
        .unwrap();
        let structured = attachment.clone().into_structured().unwrap();
        assert_eq!(
            MessageAttachment::from_structured(structured).unwrap(),
            attachment
        );
        assert!(MessageAttachment::new(
            attachment.content().clone(),
            "f".repeat(conduit_core::MAXIMUM_STRUCTURED_LEAF_BYTES + 1),
            "text/plain".into(),
        )
        .is_err());
    }
}
