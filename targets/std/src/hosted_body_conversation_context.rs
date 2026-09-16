//! Supervisor-owned, bounded current Body-context feed.

use conduit_body::BodyConversationContext;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct BodyConversationContextSource {
    shared: Arc<Mutex<SourceState>>,
}

struct SourceState {
    basis: SourceBasis,
    encoded: Box<[u8]>,
    encoded_len: usize,
    staging: Box<[u8]>,
    fingerprint: [u8; 32],
    lost: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceBasis {
    body_sha256: [u8; 32],
    wake_sha256: [u8; 32],
    wake_sequence: u64,
    revision: u64,
}

impl SourceBasis {
    fn from_context(context: &BodyConversationContext) -> Self {
        Self {
            body_sha256: Sha256::digest(context.basis.body_id.as_str().as_bytes()).into(),
            wake_sha256: Sha256::digest(context.basis.wake_id.as_str().as_bytes()).into(),
            wake_sequence: context.basis.wake_sequence,
            revision: context.basis.revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyConversationContextReplacement {
    Published,
    Coalesced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyConversationContextUpdateRefusal {
    DifferentBody,
    StaleBasis,
    ConflictingBasis,
    SourceLost,
    InvalidContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BodyConversationContextPoll<T> {
    Pending,
    Current { fingerprint: [u8; 32], value: T },
    Lost,
}

impl BodyConversationContextSource {
    pub(crate) fn new(context: &BodyConversationContext) -> Result<Self, String> {
        let mut encoded = vec![0; conduit_chat::MAXIMUM_BODY_CHAT_CONTEXT_BYTES].into_boxed_slice();
        let encoded_len =
            conduit_chat::encode_body_conversation_context_slice(context, &mut encoded)
                .map_err(|error| format!("Body conversation context: {error:?}"))?;
        let fingerprint = Sha256::digest(&encoded[..encoded_len]).into();
        Ok(Self {
            shared: Arc::new(Mutex::new(SourceState {
                basis: SourceBasis::from_context(context),
                encoded,
                encoded_len,
                staging: vec![0; conduit_chat::MAXIMUM_BODY_CHAT_CONTEXT_BYTES].into_boxed_slice(),
                fingerprint,
                lost: false,
            })),
        })
    }

    pub fn replace(
        &self,
        context: &BodyConversationContext,
    ) -> Result<BodyConversationContextReplacement, BodyConversationContextUpdateRefusal> {
        let mut state = self
            .shared
            .lock()
            .expect("Body context source lock poisoned");
        if state.lost {
            return Err(BodyConversationContextUpdateRefusal::SourceLost);
        }
        let current = state.basis;
        let candidate = SourceBasis::from_context(context);
        if candidate.body_sha256 != current.body_sha256 {
            return Err(BodyConversationContextUpdateRefusal::DifferentBody);
        }
        if candidate.wake_sequence < current.wake_sequence
            || (candidate.wake_sequence == current.wake_sequence
                && candidate.revision < current.revision)
        {
            return Err(BodyConversationContextUpdateRefusal::StaleBasis);
        }
        let staging_len =
            conduit_chat::encode_body_conversation_context_slice(context, &mut state.staging)
                .map_err(|_| BodyConversationContextUpdateRefusal::InvalidContext)?;
        if candidate == current {
            return if state.staging[..staging_len] == state.encoded[..state.encoded_len] {
                Ok(BodyConversationContextReplacement::Coalesced)
            } else {
                Err(BodyConversationContextUpdateRefusal::ConflictingBasis)
            };
        }
        if candidate.wake_sequence == current.wake_sequence
            && candidate.wake_sha256 != current.wake_sha256
        {
            return Err(BodyConversationContextUpdateRefusal::ConflictingBasis);
        }
        state.basis = candidate;
        let SourceState {
            encoded, staging, ..
        } = &mut *state;
        core::mem::swap(encoded, staging);
        state.encoded_len = staging_len;
        state.fingerprint = Sha256::digest(&state.encoded[..state.encoded_len]).into();
        Ok(BodyConversationContextReplacement::Published)
    }

    pub fn mark_lost(&self) {
        self.shared
            .lock()
            .expect("Body context source lock poisoned")
            .lost = true;
    }

    pub(crate) fn poll_after<T>(
        &self,
        delivered: Option<[u8; 32]>,
        publish: impl FnOnce(&[u8]) -> T,
    ) -> BodyConversationContextPoll<T> {
        let state = self
            .shared
            .lock()
            .expect("Body context source lock poisoned");
        if state.lost {
            return BodyConversationContextPoll::Lost;
        }
        if delivered == Some(state.fingerprint) {
            return BodyConversationContextPoll::Pending;
        }
        BodyConversationContextPoll::Current {
            fingerprint: state.fingerprint,
            value: publish(&state.encoded[..state.encoded_len]),
        }
    }

    #[cfg(test)]
    fn allocation_capacity(&self) -> usize {
        let state = self
            .shared
            .lock()
            .expect("Body context source lock poisoned");
        state.encoded.len() + state.staging.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{Body, BodyConversationContextBasis, BodyConversationHost};
    use conduit_core::{ActivePlayId, CheckedFormId, HostId, PlanId, SignId, SourceDocumentId};

    fn context(revision: u64, present: bool) -> BodyConversationContext {
        let body = Body::born(
            SourceDocumentId::from("source/current-context"),
            CheckedFormId::from("checked/current-context"),
            1,
            SignId::from("sign/current-context/born"),
        )
        .unwrap();
        let (_, wake) = body
            .wake(7, SignId::from("sign/current-context/wake"))
            .unwrap();
        let body_id = wake.body_id.clone();
        let wake_id = wake.wake_id.clone();
        BodyConversationContext {
            schema: "conduit.body/conversation-context-value@2".into(),
            display_name: "Roseau".into(),
            body_id: body_id.clone(),
            wake_id: wake_id.clone(),
            wake_sequence: 7,
            basis: BodyConversationContextBasis {
                body_id,
                wake_id,
                wake_sequence: 7,
                revision,
            },
            hosts: vec![BodyConversationHost {
                host_id: HostId::from("host/latimer"),
                present,
            }],
            active_forms: vec!["form/home".into()],
            current_plan_id: None,
            active_play_id: None,
            lines: vec![],
            recent_sign_ids: vec![],
        }
    }

    #[test]
    fn replacement_is_basis_checked_coalesced_and_loss_is_distinct() {
        let source = BodyConversationContextSource::new(&context(4, true)).unwrap();
        let capacity = source.allocation_capacity();
        let first = match source.poll_after(None, |_| ()) {
            BodyConversationContextPoll::Current { fingerprint, .. } => fingerprint,
            other => panic!("initial current context not published: {other:?}"),
        };
        assert_eq!(
            source.poll_after(Some(first), |_| ()),
            BodyConversationContextPoll::Pending
        );
        assert_eq!(
            source.replace(&context(4, true)),
            Ok(BodyConversationContextReplacement::Coalesced)
        );
        assert_eq!(
            source.replace(&context(3, false)),
            Err(BodyConversationContextUpdateRefusal::StaleBasis)
        );
        assert_eq!(
            source.replace(&context(4, false)),
            Err(BodyConversationContextUpdateRefusal::ConflictingBasis)
        );
        assert_eq!(
            source.replace(&context(5, false)),
            Ok(BodyConversationContextReplacement::Published)
        );
        assert_eq!(source.allocation_capacity(), capacity);
        match source.poll_after(Some(first), <[u8]>::to_vec) {
            BodyConversationContextPoll::Current { value: encoded, .. } => {
                let replacement = conduit_chat::decode_body_conversation_context(&encoded).unwrap();
                assert_eq!(replacement.basis.revision, 5);
                assert!(!replacement.hosts[0].present);
            }
            other => panic!("replacement current context not published: {other:?}"),
        }
        source.mark_lost();
        assert_eq!(
            source.poll_after(None, |_| ()),
            BodyConversationContextPoll::Lost
        );
    }

    #[test]
    fn retained_chat_uses_replacement_host_and_plan_truth_on_the_next_turn() {
        let initial = context(4, true);
        let source = BodyConversationContextSource::new(&initial).unwrap();
        let (first_fingerprint, first_encoded) = match source.poll_after(None, <[u8]>::to_vec) {
            BodyConversationContextPoll::Current { fingerprint, value } => (fingerprint, value),
            other => panic!("initial current context not published: {other:?}"),
        };
        let mut chat = conduit_chat::BodyChatPromptState::new(&first_encoded, 4).unwrap();
        chat.record_response(b"retained history").unwrap();
        let first_request = chat.request(b"first turn").unwrap();

        let mut replacement = context(5, false);
        replacement.current_plan_id = Some(PlanId::from("plan/replacement"));
        replacement.active_play_id = Some(ActivePlayId::from("play/replacement"));
        assert_eq!(
            source.replace(&replacement),
            Ok(BodyConversationContextReplacement::Published)
        );
        let replacement_encoded = match source.poll_after(Some(first_fingerprint), <[u8]>::to_vec) {
            BodyConversationContextPoll::Current { value, .. } => value,
            other => panic!("replacement current context not published: {other:?}"),
        };
        chat.replace_context(&replacement_encoded).unwrap();
        let next_request = chat.request(b"next turn").unwrap();

        assert_eq!(first_request.context_basis.revision, 4);
        assert_eq!(next_request.context_basis.revision, 5);
        let prompt: serde_json::Value =
            serde_json::from_slice(&next_request.encoded_request).unwrap();
        assert_eq!(prompt["body"]["offline_hosts"], 1);
        assert_eq!(prompt["body"]["execution"], "playing");
        assert!(prompt.get("context").is_none());
        assert!(chat
            .history()
            .iter()
            .any(|item| item.text == "retained history"));
    }
}
