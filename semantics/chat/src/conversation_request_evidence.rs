//! Privacy-preserving evidence for the body truth consumed by a chat request.

use alloc::string::String;

use crate::ConversationRequestEvidence;

impl From<&crate::BodyChatGenerationRequest> for ConversationRequestEvidence {
    fn from(request: &crate::BodyChatGenerationRequest) -> Self {
        let mut model_context_sha256 = String::with_capacity(64);
        for byte in request.model_context_sha256 {
            use core::fmt::Write;
            write!(&mut model_context_sha256, "{byte:02x}")
                .expect("write digest to bounded String");
        }
        Self::new(
            request.request_identity.clone(),
            request.context_basis.body_id.as_str().into(),
            request.context_basis.wake_id.as_str().into(),
            request.context_basis.wake_sequence,
            request.context_basis.revision,
            model_context_sha256,
            false,
        )
        .expect("validated Body request satisfies the evidence contract")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
    use conduit_core::{CheckedFormId, SignId, SourceDocumentId};
    use conduit_form::rust_binding::NativeRustBinding;

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
        let encoded = crate::encode_body_conversation_context(&context).unwrap();
        let mut state = crate::BodyChatPromptState::new(&encoded, 2).unwrap();
        let request = state.request(b"private message").unwrap();
        let evidence = ConversationRequestEvidence::from(&request);
        let serialized = serde_json::to_string(&evidence).unwrap();
        assert_eq!(*evidence.context_revision(), 9);
        assert!(!evidence.private_prompt_retained());
        assert!(!serialized.contains("private name"));
        assert!(!serialized.contains("private message"));

        let structured = evidence.clone().into_structured().unwrap();
        assert_eq!(
            ConversationRequestEvidence::from_structured(structured).unwrap(),
            evidence
        );
    }

    #[test]
    fn evidence_rejects_values_beyond_the_owned_identity_bounds() {
        assert!(ConversationRequestEvidence::new(
            "r".repeat(83),
            "body/fixture".into(),
            "wake/fixture".into(),
            1,
            1,
            "0".repeat(64),
            false,
        )
        .is_err());
    }
}
