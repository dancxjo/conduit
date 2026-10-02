//! Privacy-preserving chat projection of the live conversation commit seams.

use alloc::string::String;
use conduit_ai::GeneratedTextFlowEvidence;

use crate::{
    ConversationRequestEvidence, LiveConversationFlowProjection, LiveConversationProjectionError,
    LiveConversationStage, LiveConversationStageKind, LiveConversationStageState,
    LiveConversationStages, RecognitionEvidenceStatus, RecognitionEvidenceView,
    SpeechCommitEvidenceView,
};

pub const LIVE_CONVERSATION_FLOW_SCHEMA: &str = "conduit.chat/live-conversation-flow@1";
pub const MAXIMUM_PRESENTED_RECOGNITION_EVENTS: usize = 32;

impl core::ops::Index<usize> for LiveConversationStages {
    type Output = LiveConversationStage;

    fn index(&self, index: usize) -> &Self::Output {
        &self.get()[index]
    }
}

pub struct LiveConversationTruth<'a> {
    pub recognition: &'a [RecognitionEvidenceView],
    pub committed_turn_identity: Option<&'a str>,
    pub context: Option<&'a ConversationRequestEvidence>,
    pub generation: Option<&'a GeneratedTextFlowEvidence>,
    pub speech: Option<&'a SpeechCommitEvidenceView>,
}

pub fn project_live_conversation_flow(
    truth: &LiveConversationTruth<'_>,
) -> Result<LiveConversationFlowProjection, LiveConversationProjectionError> {
    if truth.recognition.len() > MAXIMUM_PRESENTED_RECOGNITION_EVENTS {
        return Err(LiveConversationProjectionError::RecognitionBoundExceeded);
    }
    let stream = truth
        .recognition
        .first()
        .map(|event| event.stream_id.as_str());
    if truth
        .recognition
        .iter()
        .any(|event| Some(event.stream_id.as_str()) != stream)
    {
        return Err(LiveConversationProjectionError::RecognitionIdentityMismatch);
    }
    let provisional = truth
        .recognition
        .iter()
        .filter(|event| {
            matches!(
                event.status,
                RecognitionEvidenceStatus::Provisional | RecognitionEvidenceStatus::Revised
            )
        })
        .count() as u64;
    let committed = truth.recognition.iter().find(|event| {
        event.status == RecognitionEvidenceStatus::Committed && event.turn_identity.is_some()
    });
    if committed.and_then(|event| event.turn_identity.as_deref()) != truth.committed_turn_identity {
        return Err(LiveConversationProjectionError::CommittedTurnMismatch);
    }
    if truth.context.is_some_and(|context| {
        context.request_identity().is_empty()
            || context.body_id().is_empty()
            || context.wake_id().is_empty()
            || context.model_context_sha256().len() != 64
            || *context.private_prompt_retained()
    }) {
        return Err(LiveConversationProjectionError::InvalidContextEvidence);
    }
    if truth
        .speech
        .is_some_and(|speech| speech.stream_identity.is_empty())
    {
        return Err(LiveConversationProjectionError::InvalidSpeechEvidence);
    }

    let generation_state = truth
        .generation
        .map_or(LiveConversationStageState::Awaiting, |flow| {
            use conduit_ai::GeneratedTextFlowTerminal::*;
            match flow.terminal {
                Completed => LiveConversationStageState::Completed,
                Cancelled => LiveConversationStageState::Cancelled,
                OutputBoundExhausted | Backpressured | ProviderLost | NonMonotonic => {
                    LiveConversationStageState::Refused
                }
            }
        });
    let speech_state = truth
        .speech
        .map_or(LiveConversationStageState::Awaiting, |speech| {
            if speech.cancelled {
                LiveConversationStageState::Cancelled
            } else if speech.segment_count > 0 {
                LiveConversationStageState::Committed
            } else {
                LiveConversationStageState::Awaiting
            }
        });
    let pcm_state = truth
        .speech
        .map_or(LiveConversationStageState::Awaiting, |speech| {
            if speech.pcm_extent_bytes > 0 {
                LiveConversationStageState::Flowing
            } else if speech.cancelled {
                LiveConversationStageState::Cancelled
            } else {
                LiveConversationStageState::Awaiting
            }
        });

    Ok(LiveConversationFlowProjection {
        schema: String::from(LIVE_CONVERSATION_FLOW_SCHEMA),
        stages: LiveConversationStages::new([
            stage(
                LiveConversationStageKind::RecognitionHypothesis,
                if provisional > 0 {
                    LiveConversationStageState::Provisional
                } else {
                    LiveConversationStageState::Awaiting
                },
                stream,
                provisional,
                truth
                    .recognition
                    .iter()
                    .map(|event| u64::from(event.audio_extent_bytes))
                    .max()
                    .unwrap_or(0),
            ),
            stage(
                LiveConversationStageKind::CommittedTurn,
                if committed.is_some() {
                    LiveConversationStageState::Committed
                } else {
                    LiveConversationStageState::Awaiting
                },
                truth.committed_turn_identity,
                u64::from(committed.is_some()),
                0,
            ),
            stage(
                LiveConversationStageKind::CurrentContextBasis,
                if truth.context.is_some() {
                    LiveConversationStageState::Completed
                } else {
                    LiveConversationStageState::Awaiting
                },
                truth
                    .context
                    .map(|context| context.request_identity().as_str()),
                truth
                    .context
                    .map_or(0, |context| *context.context_revision()),
                0,
            ),
            stage(
                LiveConversationStageKind::GeneratedTextFlow,
                generation_state,
                None,
                truth.generation.map_or(0, |flow| flow.chunks),
                truth.generation.map_or(0, |flow| flow.generated_bytes),
            ),
            stage(
                LiveConversationStageKind::CommittedSpeech,
                speech_state,
                truth.speech.map(|speech| speech.stream_identity.as_str()),
                truth
                    .speech
                    .map_or(0, |speech| u64::from(speech.segment_count)),
                truth
                    .speech
                    .map_or(0, |speech| u64::from(speech.committed_text_bytes)),
            ),
            stage(
                LiveConversationStageKind::PcmFlow,
                pcm_state,
                truth.speech.map(|speech| speech.stream_identity.as_str()),
                0,
                truth.speech.map_or(0, |speech| speech.pcm_extent_bytes),
            ),
        ])
        .expect("the six authored live-conversation stages satisfy their exact collection"),
    })
}

fn stage(
    kind: LiveConversationStageKind,
    state: LiveConversationStageState,
    identity: Option<&str>,
    items: u64,
    bytes: u64,
) -> LiveConversationStage {
    LiveConversationStage {
        kind,
        state,
        identity: identity.map(String::from),
        items,
        bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_ai::GeneratedTextFlowTerminal;

    fn recognition(status: RecognitionEvidenceStatus) -> RecognitionEvidenceView {
        RecognitionEvidenceView {
            stream_id: "recognition/live-1".into(),
            status,
            audio_extent_bytes: 640,
            turn_identity: (status == RecognitionEvidenceStatus::Committed)
                .then(|| "turn/live-1".into()),
        }
    }

    #[test]
    fn distinguishes_every_semantic_commit_and_flow_boundary() {
        let recognition = [
            recognition(RecognitionEvidenceStatus::Provisional),
            recognition(RecognitionEvidenceStatus::Committed),
        ];
        let context = ConversationRequestEvidence::new(
            "request/live-1".into(),
            "body/live".into(),
            "wake/live".into(),
            2,
            9,
            "7".repeat(64),
            false,
        )
        .unwrap();
        let generation = GeneratedTextFlowEvidence {
            chunks: 3,
            generated_bytes: 42,
            terminal: GeneratedTextFlowTerminal::Completed,
            retained_private_text: false,
        };
        let speech = SpeechCommitEvidenceView {
            stream_identity: "speech/live-1".into(),
            segment_count: 2,
            committed_text_bytes: 42,
            pcm_extent_bytes: 8_192,
            cancelled: false,
        };
        let projection = project_live_conversation_flow(&LiveConversationTruth {
            recognition: &recognition,
            committed_turn_identity: Some("turn/live-1"),
            context: Some(&context),
            generation: Some(&generation),
            speech: Some(&speech),
        })
        .unwrap();

        assert_eq!(
            projection.stages[0].state,
            LiveConversationStageState::Provisional
        );
        assert_eq!(
            projection.stages[1].state,
            LiveConversationStageState::Committed
        );
        assert_eq!(projection.stages[2].items, 9);
        assert_eq!(projection.stages[3].items, 3);
        assert_eq!(projection.stages[4].items, 2);
        assert_eq!(projection.stages[5].bytes, 8_192);
    }

    #[test]
    fn refuses_a_turn_claim_without_matching_committed_evidence() {
        let evidence = [recognition(RecognitionEvidenceStatus::Provisional)];
        assert_eq!(
            project_live_conversation_flow(&LiveConversationTruth {
                recognition: &evidence,
                committed_turn_identity: Some("turn/invented"),
                context: None,
                generation: None,
                speech: None,
            }),
            Err(LiveConversationProjectionError::CommittedTurnMismatch)
        );
    }

    #[test]
    fn cancellation_remains_distinct_from_empty_pcm() {
        let speech = SpeechCommitEvidenceView {
            stream_identity: "speech/cancelled".into(),
            segment_count: 0,
            committed_text_bytes: 0,
            pcm_extent_bytes: 0,
            cancelled: true,
        };
        let projection = project_live_conversation_flow(&LiveConversationTruth {
            recognition: &[],
            committed_turn_identity: None,
            context: None,
            generation: None,
            speech: Some(&speech),
        })
        .unwrap();
        assert_eq!(
            projection.stages[4].state,
            LiveConversationStageState::Cancelled
        );
        assert_eq!(
            projection.stages[5].state,
            LiveConversationStageState::Cancelled
        );
    }
}
