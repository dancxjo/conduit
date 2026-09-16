//! Provider-neutral, bounded Body Chat prompt and conversation state.

#[cfg(feature = "form-catalog")]
use alloc::vec;
use alloc::{
    collections::VecDeque,
    format,
    string::{String, ToString},
    vec::Vec,
};
use conduit_core::CapabilityLimits;
#[cfg(feature = "form-catalog")]
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};
#[cfg(feature = "form-catalog")]
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const BODY_CHAT_PROMPT_KIND: &str = "body/chat-prompt";
pub const BODY_CONVERSATION_CONTEXT_KIND: &str = "body/conversation-context";
pub const BODY_CONVERSATION_CONTEXT_REVISION: &str = "conduit.body/conversation-context@2";
pub const BODY_CHAT_PROMPT_REVISION: &str = "conduit.body/chat-prompt@2";
pub const BODY_CHAT_FORM_KIND: &str = "body-chat";
pub const BODY_CHAT_FORM_REVISION: &str = "conduit.body/chat-form@2";
pub const MAXIMUM_BODY_CHAT_HISTORY_ITEMS: usize = 16;
pub const MAXIMUM_BODY_CHAT_MESSAGE_BYTES: usize = 4_096;
pub const MAXIMUM_BODY_CHAT_CONTEXT_BYTES: usize = 32_768;
pub const MAXIMUM_BODY_CHAT_PROMPT_BYTES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BodyChatRole {
    Human,
    Body,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BodyChatHistoryItem {
    pub role: BodyChatRole,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyChatGenerationRequest {
    pub request_identity: String,
    pub context_basis: conduit_body::BodyConversationContextBasis,
    pub context_sha256: [u8; 32],
    pub encoded_request: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyChatRefusal {
    EmptyMessage,
    MessageBoundExceeded,
    ContextBoundExceeded,
    MalformedContext,
    WrongContextSchema,
    HistoryBoundExceeded,
    PromptBoundExceeded,
    Encoding,
}

pub fn encode_body_conversation_context(
    context: &conduit_body::BodyConversationContext,
) -> Result<Vec<u8>, BodyChatRefusal> {
    let mut encoded = Vec::with_capacity(MAXIMUM_BODY_CHAT_CONTEXT_BYTES);
    encode_body_conversation_context_into(context, &mut encoded)?;
    Ok(encoded)
}

pub fn encode_body_conversation_context_into(
    context: &conduit_body::BodyConversationContext,
    encoded: &mut Vec<u8>,
) -> Result<(), BodyChatRefusal> {
    validate_context(context)?;
    if encoded.capacity() < MAXIMUM_BODY_CHAT_CONTEXT_BYTES {
        return Err(BodyChatRefusal::ContextBoundExceeded);
    }
    encoded.clear();
    let replacement = serde_json::to_vec(context).map_err(|_| BodyChatRefusal::Encoding)?;
    if replacement.len() > MAXIMUM_BODY_CHAT_CONTEXT_BYTES {
        encoded.clear();
        return Err(BodyChatRefusal::ContextBoundExceeded);
    }
    encoded.extend_from_slice(&replacement);
    Ok(())
}

pub fn decode_body_conversation_context(
    encoded: &[u8],
) -> Result<conduit_body::BodyConversationContext, BodyChatRefusal> {
    if encoded.len() > MAXIMUM_BODY_CHAT_CONTEXT_BYTES {
        return Err(BodyChatRefusal::ContextBoundExceeded);
    }
    let context = serde_json::from_slice(encoded).map_err(|_| BodyChatRefusal::MalformedContext)?;
    validate_context(&context)?;
    if serde_json::to_vec(&context).map_err(|_| BodyChatRefusal::Encoding)? != encoded {
        return Err(BodyChatRefusal::MalformedContext);
    }
    Ok(context)
}

fn validate_context(
    context: &conduit_body::BodyConversationContext,
) -> Result<(), BodyChatRefusal> {
    if context.schema != "conduit.body/conversation-context-value@2" {
        return Err(BodyChatRefusal::WrongContextSchema);
    }
    if context.basis.body_id != context.body_id
        || context.basis.wake_id != context.wake_id
        || context.basis.wake_sequence != context.wake_sequence
    {
        return Err(BodyChatRefusal::MalformedContext);
    }
    if context.display_name.is_empty()
        || context.display_name.len() > conduit_body::MAXIMUM_BODY_DISPLAY_NAME_BYTES
        || context.hosts.len() > conduit_body::MAXIMUM_CONVERSATION_HOSTS
        || context.active_forms.len() > conduit_body::MAXIMUM_CONVERSATION_FORMS
        || context.lines.len() > conduit_body::MAXIMUM_CONVERSATION_LINES
        || context.recent_sign_ids.len() > conduit_body::MAXIMUM_CONVERSATION_SIGNS
        || context.lines.iter().any(|line| line.line_id.is_empty())
    {
        return Err(BodyChatRefusal::MalformedContext);
    }
    Ok(())
}

#[derive(Serialize)]
struct Prompt<'a> {
    schema: &'static str,
    request_identity: &'a str,
    instruction: &'static str,
    current_message: &'a str,
    history: &'a VecDeque<BodyChatHistoryItem>,
    context: &'a conduit_body::BodyConversationContext,
}

#[derive(Clone, Debug)]
pub struct BodyChatPromptState {
    context: conduit_body::BodyConversationContext,
    context_sha256: [u8; 32],
    history: VecDeque<BodyChatHistoryItem>,
    maximum_history_items: usize,
}

impl BodyChatPromptState {
    pub fn new(
        encoded_context: &[u8],
        maximum_history_items: usize,
    ) -> Result<Self, BodyChatRefusal> {
        if encoded_context.len() > MAXIMUM_BODY_CHAT_CONTEXT_BYTES {
            return Err(BodyChatRefusal::ContextBoundExceeded);
        }
        if maximum_history_items == 0 || maximum_history_items > MAXIMUM_BODY_CHAT_HISTORY_ITEMS {
            return Err(BodyChatRefusal::HistoryBoundExceeded);
        }
        let context = decode_body_conversation_context(encoded_context)?;
        let context_sha256 = Sha256::digest(encoded_context).into();
        Ok(Self {
            context,
            context_sha256,
            history: VecDeque::with_capacity(maximum_history_items),
            maximum_history_items,
        })
    }

    pub fn replace_context(&mut self, encoded_context: &[u8]) -> Result<(), BodyChatRefusal> {
        let replacement = Self::new(encoded_context, self.maximum_history_items)?;
        self.context = replacement.context;
        self.context_sha256 = replacement.context_sha256;
        Ok(())
    }

    pub fn request(
        &mut self,
        message: &[u8],
    ) -> Result<BodyChatGenerationRequest, BodyChatRefusal> {
        let message = decode_message(message)?;
        let context_sha256 = self.context_sha256;
        let mut recent_history = self.history.clone();
        let (request_identity, encoded_request) = loop {
            let mut digest = Sha256::new();
            digest.update(b"conduit-body-chat-request-v1\0");
            digest.update(self.context.body_id.as_str().as_bytes());
            digest.update(self.context.wake_id.as_str().as_bytes());
            digest.update(self.context.basis.revision.to_le_bytes());
            digest.update(context_sha256);
            digest.update(message.as_bytes());
            for item in &recent_history {
                digest.update([match item.role {
                    BodyChatRole::Human => 0,
                    BodyChatRole::Body => 1,
                }]);
                digest.update(item.text.as_bytes());
            }
            let request_identity = format!("body-chat-request/{:x}", digest.finalize());
            let prompt = Prompt {
                schema: "conduit.body/chat-prompt-value@2",
                request_identity: &request_identity,
                instruction: "Answer as this Body, briefly and only from the supplied current Body truth and explicitly labeled conversation history. Never claim an action occurred merely because it was requested.",
                current_message: message,
                history: &recent_history,
                context: &self.context,
            };
            let encoded = serde_json::to_vec(&prompt).map_err(|_| BodyChatRefusal::Encoding)?;
            if encoded.len() <= MAXIMUM_BODY_CHAT_PROMPT_BYTES {
                break (request_identity, encoded);
            }
            if recent_history.pop_front().is_none() {
                return Err(BodyChatRefusal::PromptBoundExceeded);
            }
        };
        self.push(BodyChatHistoryItem {
            role: BodyChatRole::Human,
            text: message.into(),
        });
        Ok(BodyChatGenerationRequest {
            request_identity,
            context_basis: self.context.basis.clone(),
            context_sha256,
            encoded_request,
        })
    }

    pub fn record_response(&mut self, response: &[u8]) -> Result<(), BodyChatRefusal> {
        let response = decode_message(response)?;
        self.push(BodyChatHistoryItem {
            role: BodyChatRole::Body,
            text: response.into(),
        });
        Ok(())
    }

    pub fn history(&self) -> &VecDeque<BodyChatHistoryItem> {
        &self.history
    }
    fn push(&mut self, item: BodyChatHistoryItem) {
        if self.history.len() == self.maximum_history_items {
            self.history.pop_front();
        }
        self.history.push_back(item);
    }
}

fn decode_message(bytes: &[u8]) -> Result<&str, BodyChatRefusal> {
    if bytes.is_empty() {
        return Err(BodyChatRefusal::EmptyMessage);
    }
    if bytes.len() > MAXIMUM_BODY_CHAT_MESSAGE_BYTES {
        return Err(BodyChatRefusal::MessageBoundExceeded);
    }
    core::str::from_utf8(bytes).map_err(|_| BodyChatRefusal::MalformedContext)
}

#[cfg(feature = "form-catalog")]
pub fn body_chat_prompt_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(BODY_CHAT_PROMPT_KIND),
        kind_contract_revision: KindContractRevision::from(BODY_CHAT_PROMPT_REVISION),
        inputs: vec![
            port(
                "message",
                conduit_text::TEXT_VALUE_KIND,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "response",
                conduit_text::TEXT_VALUE_KIND,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "context",
                conduit_body::BODY_CONVERSATION_CONTEXT_VALUE_KIND,
                PortTemporal::Current,
            ),
        ],
        outputs: vec![
            output("request", conduit_ai::GENERATION_REQUEST_VALUE_KIND),
            output("human-message", conduit_text::TEXT_VALUE_KIND),
            output("body-message", conduit_text::TEXT_VALUE_KIND),
        ],
        configuration: vec![],
    }
}

#[cfg(feature = "form-catalog")]
pub fn body_conversation_context_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(BODY_CONVERSATION_CONTEXT_KIND),
        kind_contract_revision: KindContractRevision::from(BODY_CONVERSATION_CONTEXT_REVISION),
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("context"),
            value_kind: kind_id(conduit_body::BODY_CONVERSATION_CONTEXT_VALUE_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
        }],
        configuration: vec![],
    }
}

#[cfg(feature = "form-catalog")]
fn port(name: &str, kind: &str, temporal: PortTemporal) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction: PortDirection::Input,
        temporal,
    }
}
#[cfg(feature = "form-catalog")]
fn output(name: &str, kind: &str) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_body_chat_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert_value_kind_alias(
        "BodyConversationContext",
        kind_id(conduit_body::BODY_CONVERSATION_CONTEXT_VALUE_KIND),
    )?;
    startup.insert(KindSignature {
        kind: BODY_CHAT_PROMPT_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(body_chat_prompt_definition())
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: BODY_CONVERSATION_CONTEXT_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(body_conversation_context_definition())
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: BODY_CHAT_FORM_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(BODY_CHAT_FORM_KIND),
            kind_contract_revision: KindContractRevision::from(BODY_CHAT_FORM_REVISION),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

pub fn body_chat_prompt_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: MAXIMUM_BODY_CHAT_HISTORY_ITEMS as u16,
        max_queue_bytes: MAXIMUM_BODY_CHAT_PROMPT_BYTES as u32,
    }
}
