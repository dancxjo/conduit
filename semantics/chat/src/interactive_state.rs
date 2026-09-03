//! Finite chat application state projected as portable Presentation truth.

use alloc::{collections::VecDeque, format, string::String, vec, vec::Vec};
use conduit_presentation::{
    Presentation, PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationDisclosure, PresentationDisclosureLevel, PresentationInput, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText, UTF8_TEXT_VALUE_KIND,
};

pub const CHAT_SEND_ACTION: &str = "chat/send";
pub const CHAT_MESSAGE_INPUT: &str = "chat/message-input";
pub const CHAT_MESSAGE_TARGET: &str = "chat/message-entry";
pub const MAXIMUM_CHAT_MESSAGE_BYTES: u32 = 256;
pub const MAXIMUM_CHAT_HISTORY_ITEMS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPresentationConfiguration {
    pub title: String,
    pub history_label: String,
    pub input_label: String,
    pub submit_label: String,
    pub status_label: String,
    pub maximum_message_bytes: u32,
    pub maximum_history_items: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatStateRefusal {
    InvalidConfiguration,
    EmptyMessage,
    OversizeMessage,
    MalformedMessage,
    SequenceExhausted,
    InvalidPresentation,
}

#[derive(Debug, Clone)]
pub struct ChatPresentationState {
    configuration: ChatPresentationConfiguration,
    connection: ChatConnectionState,
    revision: u64,
    next_message: u64,
    messages: VecDeque<(u64, String)>,
}

impl ChatPresentationConfiguration {
    pub fn validate(&self) -> Result<(), ChatStateRefusal> {
        if self.title.is_empty()
            || self.history_label.is_empty()
            || self.input_label.is_empty()
            || self.submit_label.is_empty()
            || self.status_label.is_empty()
            || self.maximum_message_bytes == 0
            || self.maximum_message_bytes > MAXIMUM_CHAT_MESSAGE_BYTES
            || self.maximum_history_items == 0
            || self.maximum_history_items > MAXIMUM_CHAT_HISTORY_ITEMS
        {
            return Err(ChatStateRefusal::InvalidConfiguration);
        }
        Ok(())
    }
}

impl ChatPresentationState {
    pub fn new(configuration: ChatPresentationConfiguration) -> Result<Self, ChatStateRefusal> {
        configuration.validate()?;
        let maximum_history_items = configuration.maximum_history_items;
        Ok(Self {
            configuration,
            connection: ChatConnectionState::Connecting,
            revision: 1,
            next_message: 0,
            messages: VecDeque::with_capacity(maximum_history_items),
        })
    }

    pub fn set_connection(
        &mut self,
        connection: ChatConnectionState,
    ) -> Result<(), ChatStateRefusal> {
        if self.connection != connection {
            self.connection = connection;
            self.bump_revision()?;
        }
        Ok(())
    }

    pub fn receive(&mut self, bytes: &[u8]) -> Result<(), ChatStateRefusal> {
        if bytes.is_empty() {
            return Err(ChatStateRefusal::EmptyMessage);
        }
        if bytes.len() > self.configuration.maximum_message_bytes as usize {
            return Err(ChatStateRefusal::OversizeMessage);
        }
        let message = core::str::from_utf8(bytes)
            .map_err(|_| ChatStateRefusal::MalformedMessage)?
            .into();
        let sequence = self.next_message;
        self.next_message = self
            .next_message
            .checked_add(1)
            .ok_or(ChatStateRefusal::SequenceExhausted)?;
        if self.messages.len() == self.configuration.maximum_history_items {
            self.messages.pop_front();
        }
        self.messages.push_back((sequence, message));
        self.bump_revision()
    }

    pub fn presentation(&self) -> Result<Presentation, ChatStateRefusal> {
        let mut subjects = vec![
            subject(
                "chat/document",
                PresentationRole::Document,
                &self.configuration.title,
            ),
            subject(
                "chat/history",
                PresentationRole::Collection,
                &self.configuration.history_label,
            ),
            subject(
                CHAT_MESSAGE_TARGET,
                PresentationRole::TextEntry,
                &self.configuration.input_label,
            ),
            subject(
                "chat/status",
                PresentationRole::Status,
                &self.configuration.status_label,
            ),
        ];
        let mut relationships = vec![
            contains("chat/document", "chat/history"),
            contains("chat/document", CHAT_MESSAGE_TARGET),
            contains("chat/document", "chat/status"),
        ];
        let mut text = vec![PresentationText {
            subject: "chat/status".into(),
            text: match self.connection {
                ChatConnectionState::Connecting => "Connecting",
                ChatConnectionState::Connected => "Connected",
                ChatConnectionState::Disconnected => "Disconnected",
            }
            .into(),
        }];
        for (sequence, message) in &self.messages {
            let identity = format!("chat/message/{sequence}");
            subjects.push(subject(&identity, PresentationRole::Item, "Chat message"));
            relationships.push(contains("chat/history", &identity));
            text.push(PresentationText {
                subject: identity,
                text: message.clone(),
            });
        }
        Presentation::new_with_interactions(
            self.revision,
            empty_basis(),
            subjects,
            relationships,
            vec![
                PresentationProperty {
                    subject: "chat/history".into(),
                    name: "maximum-items".into(),
                    value: PresentationPropertyValue::Count(
                        self.configuration.maximum_history_items as u64,
                    ),
                },
                PresentationProperty {
                    subject: CHAT_MESSAGE_TARGET.into(),
                    name: "maximum-bytes".into(),
                    value: PresentationPropertyValue::Count(
                        self.configuration.maximum_message_bytes.into(),
                    ),
                },
            ],
            text,
            vec![PresentationAction {
                identity: CHAT_SEND_ACTION.into(),
                intent: CHAT_SEND_ACTION.into(),
                target: CHAT_MESSAGE_TARGET.into(),
                label: self.configuration.submit_label.clone(),
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: match self.connection {
                    ChatConnectionState::Connected => PresentationActionAvailability::Available,
                    ChatConnectionState::Connecting => unavailable("connection/connecting"),
                    ChatConnectionState::Disconnected => unavailable("connection/disconnected"),
                },
            }],
            vec![PresentationInput {
                identity: CHAT_MESSAGE_INPUT.into(),
                target: CHAT_MESSAGE_TARGET.into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                maximum_bytes: self.configuration.maximum_message_bytes,
                allow_empty: false,
                label: self.configuration.input_label.clone(),
                accessibility_name: self.configuration.input_label.clone(),
                submit_action: CHAT_SEND_ACTION.into(),
            }],
            vec![PresentationDisclosure {
                subject: "chat/document".into(),
                level: PresentationDisclosureLevel::Primary,
            }],
        )
        .map_err(|_| ChatStateRefusal::InvalidPresentation)
    }

    pub fn history_len(&self) -> usize {
        self.messages.len()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn bump_revision(&mut self) -> Result<(), ChatStateRefusal> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(ChatStateRefusal::SequenceExhausted)?;
        Ok(())
    }
}

fn subject(identity: &str, role: PresentationRole, label: &str) -> PresentationSubject {
    PresentationSubject {
        identity: identity.into(),
        role,
        label: label.into(),
        accessibility_name: label.into(),
    }
}

fn contains(source: &str, target: &str) -> PresentationRelationship {
    PresentationRelationship {
        source: source.into(),
        target: target.into(),
        kind: PresentationRelationshipKind::Contains,
    }
}

fn unavailable(reason_code: &str) -> PresentationActionAvailability {
    PresentationActionAvailability::Unavailable {
        reason_code: reason_code.into(),
        explanation: "The semantic chat connection is not available.".into(),
    }
}

fn empty_basis() -> PresentationBasis {
    PresentationBasis {
        body_id: None,
        wake_id: None,
        source_document_id: None,
        checked_form_id: None,
        expanded_form_id: None,
        plan_id: None,
        active_play_id: None,
        sign_ids: Vec::new(),
    }
}
