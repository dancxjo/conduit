#![cfg(feature = "conversation-flow")]

use conduit_chat::{
    LiveConversationFlowProjection, LiveConversationStage, LiveConversationStageKind,
    LiveConversationStageState, LiveConversationStages, RecognitionEvidenceStatus,
    RecognitionEvidenceView, SpeechCommitEvidenceView,
};
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn live_conversation_projection_family_is_native_and_bounded() {
    let recognition = RecognitionEvidenceView::new(
        640,
        RecognitionEvidenceStatus::Committed,
        "recognition/live-1".into(),
        Some("turn/live-1".into()),
    )
    .unwrap();
    assert_eq!(
        RecognitionEvidenceView::from_structured(recognition.clone().into_structured().unwrap()),
        Ok(recognition)
    );

    let speech =
        SpeechCommitEvidenceView::new(false, 32, 8_192, 2, "speech/live-1".into()).unwrap();
    assert_eq!(
        SpeechCommitEvidenceView::from_structured(speech.clone().into_structured().unwrap()),
        Ok(speech)
    );

    let stage = LiveConversationStage::new(
        8_192,
        Some("speech/live-1".into()),
        2,
        LiveConversationStageKind::CommittedSpeech,
        LiveConversationStageState::Committed,
    )
    .unwrap();
    let stages = LiveConversationStages::new(core::array::from_fn(|_| stage.clone())).unwrap();
    let projection =
        LiveConversationFlowProjection::new("conduit.chat/live-conversation-flow@1".into(), stages)
            .unwrap();
    assert_eq!(
        LiveConversationFlowProjection::from_structured(
            projection.clone().into_structured().unwrap()
        ),
        Ok(projection)
    );
}
