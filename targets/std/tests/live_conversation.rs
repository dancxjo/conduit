//! Canonical live-conversation Form conformance across semantic owners.

use conduit_core::PortTemporal;
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

#[test]
fn canonical_live_conversation_is_one_reviewed_temporal_form() {
    let source = include_str!("../../../forms/live-conversation/main.conduit");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    startup
        .insert_value_kind_alias(
            "PcmFrames",
            conduit_core::kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
        )
        .unwrap();
    startup
        .insert_value_kind_alias(
            "ChatMessage",
            conduit_core::kind_id(conduit_tongues::CHAT_MESSAGE_VALUE_KIND),
        )
        .unwrap();
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profile).unwrap();
    conduit_tongues::install_speech_catalogs(&mut startup, &mut profile).unwrap();
    conduit_chat::install_body_chat_catalog(&mut startup, &mut profile).unwrap();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    conduit_ai::install_model_text_catalog(&mut startup, &mut profile).unwrap();

    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "live-conversation", &profile).unwrap();
    assert_eq!(
        authored.face.inputs()[0].temporal,
        PortTemporal::Flow { closes: true }
    );
    assert_eq!(
        authored.face.outputs()[0].temporal,
        PortTemporal::Flow { closes: true }
    );
    let kinds = authored
        .expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.as_str())
        .collect::<Vec<_>>();
    for expected in [
        conduit_tongues::STREAMING_SPEECH_RECOGNIZE_KIND,
        conduit_tongues::COMMIT_RECOGNIZED_TURN_KIND,
        conduit_tongues::COMMITTED_TURN_TO_TEXT_KIND,
        conduit_chat::BODY_CONVERSATION_CONTEXT_KIND,
        conduit_chat::BODY_CHAT_PROMPT_KIND,
        conduit_ai::LLM_STREAM_GENERATE_KIND,
        conduit_ai::GENERATED_CHUNK_TO_TEXT_KIND,
        conduit_tongues::SPEECH_COMMIT_KIND,
        conduit_tongues::SPEECH_SYNTHESIZE_STREAM_KIND,
    ] {
        assert!(
            kinds.contains(&expected),
            "canonical Form omitted {expected}"
        );
    }
    let lower = source.to_ascii_lowercase();
    for forbidden in [
        "whisper",
        "piper",
        "ollama",
        "alsa",
        "webaudio",
        "browser",
        "host",
        "socket",
        "conduit-test",
        "wav",
    ] {
        assert!(
            !lower.contains(forbidden),
            "canonical Form contains {forbidden}"
        );
    }
}
