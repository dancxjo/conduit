//! Deterministic fake-provider execution of the checked canonical live Form.

use conduit_ai::{
    project_generated_chunk_text, BoundedGeneratedTextFlow, GeneratedTextChunk,
    GeneratedTextFlowTerminal,
};
use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};
use conduit_tongues::{
    committed_user_message, project_committed_turn_text, StreamingSpeechCommitter,
};
use speaking::{SegmentId, StreamEvent, TextRole};

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

#[test]
fn committed_external_barge_in_cancels_generation_and_pending_speech_exactly() {
    let mut generation = BoundedGeneratedTextFlow::new(256).unwrap();
    let first = GeneratedTextChunk {
        sequence: 0,
        text: "Already audible. unfinished".into(),
    };
    generation.admit(&first).unwrap();
    let mut speech = StreamingSpeechCommitter::new("answer/barge-in").unwrap();
    let committed_segments = speech.push(&first.text).unwrap();
    assert_eq!(committed_segments.len(), 1);
    let committed_pcm = fake_tts(&committed_segments[0].text);
    assert!(!committed_pcm.is_empty());
    assert_eq!(speech.pending_text(), "unfinished");

    let provisional = StreamEvent::PartialHypothesis {
        role: TextRole::Recognition,
        segment_id: SegmentId("recognition/barge-in".into()),
        text: "Wait".into(),
        confidence: None,
    };
    assert!(committed_user_message(&provisional).unwrap().is_none());
    assert_eq!(speech.pending_text(), "unfinished");

    let committed = StreamEvent::CommittedSegment {
        role: TextRole::Recognition,
        segment_id: SegmentId("recognition/barge-in".into()),
        text: "Wait.".into(),
        words: Vec::new(),
        language: None,
        speaker_id: None,
        confidence: None,
    };
    let message = committed_user_message(&committed)
        .unwrap()
        .expect("committed external recognition requests barge-in");
    assert_eq!(message.text, "Wait.");
    assert!(!message.turn_identity.is_empty());

    let generation_evidence = generation.finish(GeneratedTextFlowTerminal::Cancelled);
    speech.cancel();
    let speech_evidence = speech.evidence(Some(1), None, committed_pcm.len() as u64);
    assert_eq!(
        generation_evidence.terminal,
        GeneratedTextFlowTerminal::Cancelled
    );
    assert_eq!(generation_evidence.chunks, 1);
    assert!(speech.pending_text().is_empty());
    assert!(speech_evidence.cancelled);
    assert_eq!(speech_evidence.segment_count, 1);
    assert_eq!(
        speech_evidence.committed_text_bytes,
        committed_segments[0].text.len() as u32
    );
    assert_eq!(speech_evidence.pcm_extent_bytes, committed_pcm.len() as u64);
    assert!(speech.push("must not resume").is_err());
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
    conduit_semantic_catalog::install_audio_capture_push_to_talk_catalog(
        &mut startup,
        &mut profile,
    )
    .unwrap();
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

    let provisional = StreamEvent::PartialHypothesis {
        role: TextRole::Recognition,
        segment_id: SegmentId("recognition/fake-one".into()),
        text: "How is".into(),
        confidence: None,
    };
    assert!(committed_user_message(&provisional).unwrap().is_none());

    let committed = StreamEvent::CommittedSegment {
        role: TextRole::Recognition,
        segment_id: SegmentId("recognition/fake-one".into()),
        text: "How is the Body?".into(),
        words: Vec::new(),
        language: None,
        speaker_id: None,
        confidence: None,
    };
    let message = committed_user_message(&committed)
        .unwrap()
        .expect("committed recognition becomes one Body turn");
    let user_text = project_committed_turn_text(&message).unwrap();
    assert_eq!(user_text, "How is the Body?");
    assert!(!message.turn_identity.is_empty());

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
