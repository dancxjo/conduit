use conduit_body::{
    Body, BodyConversationContext, BodyConversationContextBasis, BodyConversationHost,
};
use conduit_chat::{
    install_body_chat_catalog, BodyChatPromptState, BodyChatRole, BODY_CHAT_PROMPT_KIND,
    MAXIMUM_BODY_CHAT_HISTORY_ITEMS, MAXIMUM_BODY_CHAT_PROMPT_BYTES,
};
use conduit_core::{CheckedFormId, HostId, SignId, SourceDocumentId};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

fn context() -> BodyConversationContext {
    let body = Body::born(
        SourceDocumentId::from("source/roseau"),
        CheckedFormId::from("checked/roseau"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let (body, wake) = body.wake(4, SignId::from("sign/wake")).unwrap();
    BodyConversationContext {
        schema: "conduit.body/conversation-context-value@2".into(),
        display_name: "Roseau".into(),
        body_id: body.body_id.clone(),
        wake_id: wake.wake_id.clone(),
        wake_sequence: 4,
        basis: BodyConversationContextBasis {
            body_id: body.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            wake_sequence: 4,
            revision: 0,
        },
        hosts: vec![BodyConversationHost {
            host_id: HostId::from("Latimer"),
            present: true,
        }],
        active_forms: vec!["Tour".into()],
        current_plan_id: None,
        active_play_id: None,
        lines: vec![],
        recent_sign_ids: vec![],
    }
}

#[test]
fn prompt_uses_bounded_owned_history_and_current_body_truth() {
    let encoded = serde_json::to_vec(&context()).unwrap();
    let mut state = BodyChatPromptState::new(&encoded, 4).unwrap();
    let first = state.request(b"What are you doing?").unwrap();
    let prompt = String::from_utf8(first.encoded_request).unwrap();
    assert!(prompt.contains("Roseau"));
    assert!(prompt.contains("Latimer"));
    assert!(prompt.contains("Tour"));
    assert!(!prompt.to_ascii_lowercase().contains("ollama"));
    state.record_response(b"I am running the Tour.").unwrap();
    for index in 0..MAXIMUM_BODY_CHAT_HISTORY_ITEMS + 2 {
        state
            .request(format!("message {index}").as_bytes())
            .unwrap();
    }
    assert_eq!(state.history().len(), 4);
    assert!(matches!(
        state.history().back().unwrap().role,
        BodyChatRole::Human
    ));
}

#[test]
fn each_request_binds_the_context_basis_it_consumed_without_erasing_history() {
    let initial = context();
    let encoded = conduit_chat::encode_body_conversation_context(&initial).unwrap();
    let mut state = BodyChatPromptState::new(&encoded, 4).unwrap();
    state.record_response(b"Earlier response").unwrap();
    let first = state.request(b"first").unwrap();
    let mut replacement = initial;
    replacement.basis.revision = 1;
    replacement.hosts[0].present = false;
    let replacement = conduit_chat::encode_body_conversation_context(&replacement).unwrap();
    state.replace_context(&replacement).unwrap();
    let second = state.request(b"second").unwrap();
    assert_eq!(first.context_basis.revision, 0);
    assert_eq!(second.context_basis.revision, 1);
    assert_ne!(first.context_sha256, second.context_sha256);
    assert_ne!(first.request_identity, second.request_identity);
    assert!(state
        .history()
        .iter()
        .any(|item| item.text == "Earlier response"));
}

#[test]
fn prompt_keeps_owned_history_but_sends_only_the_recent_suffix_that_fits() {
    let encoded = serde_json::to_vec(&context()).unwrap();
    let mut state = BodyChatPromptState::new(&encoded, MAXIMUM_BODY_CHAT_HISTORY_ITEMS).unwrap();
    for index in 0..MAXIMUM_BODY_CHAT_HISTORY_ITEMS {
        let message = format!("human-{index:02}-{}", "h".repeat(180));
        state.request(message.as_bytes()).unwrap();
        let response = format!("body-{index:02}-{}", "b".repeat(180));
        state.record_response(response.as_bytes()).unwrap();
    }
    let request = state.request(b"what is current?").unwrap();
    let prompt = String::from_utf8(request.encoded_request).unwrap();
    assert!(prompt.len() <= MAXIMUM_BODY_CHAT_PROMPT_BYTES);
    assert!(!prompt.contains("human-00-"));
    assert!(prompt.contains("body-15-"));
    assert_eq!(state.history().len(), MAXIMUM_BODY_CHAT_HISTORY_ITEMS);
}

#[test]
fn canonical_body_chat_is_an_ordinary_checked_form() {
    let source = include_str!("../../../forms/body-chat/main.conduit");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profile).unwrap();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    conduit_ai::install_model_text_catalog(&mut startup, &mut profile).unwrap();
    install_body_chat_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "body-chat", &profile).unwrap();
    assert!(authored.input_bindings.is_empty());
    assert!(authored
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == BODY_CHAT_PROMPT_KIND));
    assert!(authored
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == conduit_chat::BODY_CONVERSATION_CONTEXT_KIND));
    for forbidden in [
        "ollama",
        "http",
        "websocket",
        "microphone",
        "speaker",
        "dom",
    ] {
        assert!(!source.to_ascii_lowercase().contains(forbidden));
    }
}
