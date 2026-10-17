use conduit_chat::{
    ChatConnectionState, ChatFaceConfiguration, ChatFaceState, ChatStateRefusal,
    CHAT_MESSAGE_INPUT, MAXIMUM_CHAT_HISTORY_ITEMS, MAXIMUM_CHAT_MESSAGE_BYTES,
};
use conduit_face::{FaceActionAvailability, FaceRole};

fn configuration(input_label: &str) -> ChatFaceConfiguration {
    ChatFaceConfiguration {
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
    let mut state = ChatFaceState::new(configuration("Message")).unwrap();
    assert!(matches!(
        state.face().unwrap().actions[0].availability,
        FaceActionAvailability::Unavailable { .. }
    ));
    state
        .set_connection(ChatConnectionState::Connected)
        .unwrap();
    for index in 0..=MAXIMUM_CHAT_HISTORY_ITEMS {
        state
            .receive(format!("message-{index}").as_bytes())
            .unwrap();
    }
    let face = state.face().unwrap();
    assert_eq!(state.history_len(), MAXIMUM_CHAT_HISTORY_ITEMS);
    assert_eq!(face.actions[0].arguments[0].name, CHAT_MESSAGE_INPUT);
    assert_eq!(
        face.actions[0].arguments[0].contract.maximum_bytes,
        MAXIMUM_CHAT_MESSAGE_BYTES
    );
    assert_eq!(face.actions[0].arguments[0].value_name, "Message");
    assert!(
        face.subjects
            .iter()
            .filter(|subject| subject.role == FaceRole::Item)
            .count()
            <= MAXIMUM_CHAT_HISTORY_ITEMS
    );
    assert_eq!(
        face.actions[0].availability,
        FaceActionAvailability::Available
    );
    let linear = conduit_face::render_linear_face(&face).unwrap();
    let text = linear.lines.join("\n");
    assert!(!text.contains("browser"));
    assert!(!text.contains("DOM"));
}

#[test]
fn source_label_change_alone_changes_face_identity_and_argument_semantics() {
    let first = ChatFaceState::new(configuration("Message"))
        .unwrap()
        .face()
        .unwrap();
    let second = ChatFaceState::new(configuration("Say something"))
        .unwrap()
        .face()
        .unwrap();
    assert_ne!(first.identity, second.identity);
    assert_eq!(second.actions[0].arguments[0].value_name, "Say something");
}

#[test]
fn malformed_empty_and_oversize_messages_remain_distinct() {
    let mut state = ChatFaceState::new(configuration("Message")).unwrap();
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
