//! Privacy-preserving evidence for the Body truth consumed by a chat request.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversationRequestEvidence {
    pub request_identity: String,
    pub body_id: String,
    pub wake_id: String,
    pub wake_sequence: u64,
    pub context_revision: u64,
    pub context_sha256: String,
    pub private_prompt_retained: bool,
}

impl From<&conduit_chat::BodyChatGenerationRequest> for ConversationRequestEvidence {
    fn from(request: &conduit_chat::BodyChatGenerationRequest) -> Self {
        let mut context_sha256 = String::with_capacity(64);
        for byte in request.context_sha256 {
            use core::fmt::Write;
            write!(&mut context_sha256, "{byte:02x}").expect("write digest to bounded String");
        }
        Self {
            request_identity: request.request_identity.clone(),
            body_id: request.context_basis.body_id.as_str().into(),
            wake_id: request.context_basis.wake_id.as_str().into(),
            wake_sequence: request.context_basis.wake_sequence,
            context_revision: request.context_basis.revision,
            context_sha256,
            private_prompt_retained: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
    use conduit_core::{CheckedFormId, SignId, SourceDocumentId};

    #[test]
    fn evidence_names_exact_context_basis_without_retaining_prompt_text() {
        let body = Body::born(
            SourceDocumentId::from("source/evidence"),
            CheckedFormId::from("checked/evidence"),
            1,
            SignId::from("sign/evidence/born"),
        )
        .unwrap();
        let (_, wake) = body.wake(3, SignId::from("sign/evidence/wake")).unwrap();
        let body_id = wake.body_id.clone();
        let wake_id = wake.wake_id.clone();
        let context = BodyConversationContext {
            schema: "conduit.body/conversation-context-value@2".into(),
            display_name: "private name".into(),
            body_id: body_id.clone(),
            wake_id: wake_id.clone(),
            wake_sequence: 3,
            basis: BodyConversationContextBasis {
                body_id,
                wake_id,
                wake_sequence: 3,
                revision: 9,
            },
            hosts: vec![],
            active_forms: vec![],
            current_plan_id: None,
            active_play_id: None,
            lines: vec![],
            recent_sign_ids: vec![],
        };
        let encoded = conduit_chat::encode_body_conversation_context(&context).unwrap();
        let mut state = conduit_chat::BodyChatPromptState::new(&encoded, 2).unwrap();
        let request = state.request(b"private message").unwrap();
        let evidence = ConversationRequestEvidence::from(&request);
        let serialized = serde_json::to_string(&evidence).unwrap();
        assert_eq!(evidence.context_revision, 9);
        assert!(!evidence.private_prompt_retained);
        assert!(!serialized.contains("private name"));
        assert!(!serialized.contains("private message"));
    }
}
