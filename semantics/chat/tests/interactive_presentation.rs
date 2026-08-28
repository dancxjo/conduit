use conduit_chat::{
    ChatConnectionState, ChatPresentationConfiguration, ChatPresentationState, ChatStateRefusal,
    CHAT_MESSAGE_INPUT, MAXIMUM_CHAT_HISTORY_ITEMS, MAXIMUM_CHAT_MESSAGE_BYTES,
};
use conduit_presentation::{PresentationActionAvailability, PresentationRole};

fn configuration(input_label: &str) -> ChatPresentationConfiguration {
    ChatPresentationConfiguration {
        title: "Conduit Webchat".into(),
        history_label: "Chat history".into(),
        input_label: input_label.into(),
        submit_label: "Send".into(),
        status_label: "Connection".into(),
        maximum_message_bytes: MAXIMUM_CHAT_MESSAGE_BYTES,
        maximum_history_items: MAXIMUM_CHAT_HISTORY_ITEMS,
    }
}

#[test]
fn authored_state_is_the_bounded_semantic_ui_truth() {
    let mut state = ChatPresentationState::new(configuration("Message")).unwrap();
    assert!(matches!(
        state.presentation().unwrap().actions[0].availability,
        PresentationActionAvailability::Unavailable { .. }
    ));
    state
        .set_connection(ChatConnectionState::Connected)
        .unwrap();
    for index in 0..=MAXIMUM_CHAT_HISTORY_ITEMS {
        state
            .receive(format!("message-{index}").as_bytes())
            .unwrap();
    }
    let presentation = state.presentation().unwrap();
    assert_eq!(state.history_len(), MAXIMUM_CHAT_HISTORY_ITEMS);
    assert_eq!(presentation.inputs[0].identity, CHAT_MESSAGE_INPUT);
    assert_eq!(
        presentation.inputs[0].maximum_bytes,
        MAXIMUM_CHAT_MESSAGE_BYTES
    );
    assert_eq!(presentation.inputs[0].label, "Message");
    assert!(
        presentation
            .subjects
            .iter()
            .filter(|subject| subject.role == PresentationRole::Item)
            .count()
            <= MAXIMUM_CHAT_HISTORY_ITEMS
    );
    assert_eq!(
        presentation.actions[0].availability,
        PresentationActionAvailability::Available
    );
    let linear = conduit_presentation::render_linear_presentation(&presentation).unwrap();
    let text = linear.lines.join("\n");
    assert!(!text.contains("browser"));
    assert!(!text.contains("DOM"));
}

#[test]
fn source_label_change_alone_changes_presentation_identity_and_input_semantics() {
    let first = ChatPresentationState::new(configuration("Message"))
        .unwrap()
        .presentation()
        .unwrap();
    let second = ChatPresentationState::new(configuration("Say something"))
        .unwrap()
        .presentation()
        .unwrap();
    assert_ne!(first.identity, second.identity);
    assert_eq!(second.inputs[0].label, "Say something");
}

#[test]
fn malformed_empty_and_oversize_messages_remain_distinct() {
    let mut state = ChatPresentationState::new(configuration("Message")).unwrap();
    assert_eq!(state.receive(&[]), Err(ChatStateRefusal::EmptyMessage));
    assert_eq!(
        state.receive(&[0xff]),
        Err(ChatStateRefusal::MalformedMessage)
    );
    assert_eq!(
        state.receive(&vec![b'x'; MAXIMUM_CHAT_MESSAGE_BYTES as usize + 1]),
        Err(ChatStateRefusal::OversizeMessage)
    );
}
