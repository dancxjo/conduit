//! Deterministic fake-provider execution of the checked canonical live Form.

use conduit_ai::{
    project_generated_chunk_text, BoundedGeneratedTextFlow, GeneratedTextChunk,
    GeneratedTextFlowTerminal,
};
use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};
use conduit_tongues::{
    project_committed_turn_text, RecognitionEvent, RecognitionEventStatus, RecognizedTurnCommitter,
    SpeechOrigin, StreamingSpeechCommitter, TurnCommitOutcome,
};

fn context() -> BodyConversationContext {
    let body = Body::born(
        SourceDocumentId::from("source/live-conversation-execution"),
        CheckedFormId::from("checked/live-conversation-execution"),
        1,
        SignId::from("sign/live-conversation-execution/born"),
    )
    .unwrap();
    let (_, wake) = body
        .wake(2, SignId::from("sign/live-conversation-execution/wake"))
        .unwrap();
    BodyConversationContext {
        schema: "conduit.body/conversation-context-value@2".into(),
        display_name: "Deterministic Body".into(),
        body_id: wake.body_id.clone(),
        wake_id: wake.wake_id.clone(),
        wake_sequence: 2,
        basis: BodyConversationContextBasis {
            body_id: wake.body_id,
            wake_id: wake.wake_id,
            wake_sequence: 2,
            revision: 3,
        },
        hosts: vec![],
        active_forms: vec!["form/live-conversation".into()],
        current_plan_id: None,
        active_play_id: None,
        lines: vec![],
        recent_sign_ids: vec![],
    }
}

fn fake_tts(segment: &str) -> Vec<u8> {
    segment
        .bytes()
        .flat_map(|byte| [byte, 0])
        .collect::<Vec<_>>()
}

#[test]
fn checked_live_form_executes_one_streaming_turn_with_fake_asr_model_and_tts() {
    let source = include_str!("../../../forms/live-conversation/main.conduit");
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    startup
        .insert_value_kind_alias(
            "PcmFrames",
            conduit_core::kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
        )
        .unwrap();
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profile).unwrap();
    conduit_tongues::install_speech_catalogs(&mut startup, &mut profile).unwrap();
    conduit_chat::install_body_chat_catalog(&mut startup, &mut profile).unwrap();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    conduit_ai::install_model_text_catalog(&mut startup, &mut profile).unwrap();
    let checked =
        conduit_form::check_syntax_document(&conduit_form::parse_syntax_document(source), &startup)
            .unwrap();
    let authored =
        conduit_form::expand_canonical_form_for_authoring(&checked, "live-conversation", &profile)
            .unwrap();
    let expanded = authored.expanded;
    let mut exact_kinds = expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.as_str())
        .collect::<Vec<_>>();
    exact_kinds.sort_unstable();
    let mut expected_kinds = vec![
        "speech/recognize-stream",
        "speech/commit-recognized-turn",
        "speech/committed-turn-to-text",
        "body/conversation-context",
        "body/chat-prompt",
        "llm/generate-stream",
        "llm/generated-chunk-to-text",
        "speech/commit-generated-text",
        "speech/synthesize-stream",
    ];
    expected_kinds.sort_unstable();
    assert_eq!(exact_kinds, expected_kinds);

    let mut recognition = RecognizedTurnCommitter::new("recognition/fake-one");
    let provisional = RecognitionEvent {
        stream_id: "recognition/fake-one".into(),
        sequence: 0,
        status: RecognitionEventStatus::Provisional,
        origin: SpeechOrigin::External,
        text: Some("How is".into()),
        audio_extent_bytes: 512,
        elapsed_milliseconds: 20,
        provider_identity: "deterministic-asr@1".into(),
    };
    let (outcome, message, _) = recognition.accept(&provisional).unwrap();
    assert_eq!(outcome, TurnCommitOutcome::Provisional);
    assert!(message.is_none());
    let committed = RecognitionEvent {
        sequence: 1,
        status: RecognitionEventStatus::Committed,
        text: Some("How is the Body?".into()),
        audio_extent_bytes: 1024,
        elapsed_milliseconds: 40,
        ..provisional
    };
    let (outcome, message, recognition_evidence) = recognition.accept(&committed).unwrap();
    assert_eq!(outcome, TurnCommitOutcome::Message);
    let message = message.unwrap();
    let user_text = project_committed_turn_text(&message).unwrap();
    assert_eq!(user_text, "How is the Body?");
    assert!(recognition_evidence.turn_identity.is_some());

    let encoded_context = conduit_chat::encode_body_conversation_context(&context()).unwrap();
    let mut chat = conduit_chat::BodyChatPromptState::new(&encoded_context, 4).unwrap();
    let request = chat.request(user_text.as_bytes()).unwrap();
    assert_eq!(request.context_basis.revision, 3);
    assert!(request
        .encoded_request
        .windows(user_text.len())
        .any(|window| window == user_text.as_bytes()));

    let chunks = [
        GeneratedTextChunk {
            sequence: 0,
            text: "The Body is awake. ".into(),
        },
        GeneratedTextChunk {
            sequence: 1,
            text: "Its current context is bounded.".into(),
        },
    ];
    let mut generation = BoundedGeneratedTextFlow::new(256).unwrap();
    let mut speech = StreamingSpeechCommitter::new("answer/fake-one").unwrap();
    let mut submitted_text = String::new();
    let mut pcm_flow = Vec::new();
    let mut first_audio_before_generation_closed = false;
    for (index, chunk) in chunks.iter().enumerate() {
        generation.admit(chunk).unwrap();
        let delta = project_generated_chunk_text(chunk).unwrap();
        for segment in speech.push(delta).unwrap() {
            submitted_text.push_str(&segment.text);
            pcm_flow.push(fake_tts(&segment.text));
            if index + 1 < chunks.len() {
                first_audio_before_generation_closed = true;
            }
        }
    }
    for segment in speech.close().unwrap() {
        submitted_text.push_str(&segment.text);
        pcm_flow.push(fake_tts(&segment.text));
    }
    let generation_evidence = generation.finish(GeneratedTextFlowTerminal::Completed);
    let expected = chunks
        .iter()
        .map(|chunk| chunk.text.as_str())
        .collect::<String>();
    assert_eq!(submitted_text, expected);
    assert!(first_audio_before_generation_closed);
    assert!(!pcm_flow.is_empty());
    assert!(pcm_flow.iter().all(|block| !block.is_empty()));
    assert_eq!(generation_evidence.chunks, 2);
    assert_eq!(generation_evidence.generated_bytes, expected.len() as u64);
    let speech_evidence = speech.evidence(
        Some(1),
        Some(1),
        pcm_flow.iter().map(Vec::len).sum::<usize>() as u64,
    );
    assert_eq!(speech_evidence.committed_text_bytes, expected.len() as u32);
    assert!(!speech_evidence.cancelled);
}
