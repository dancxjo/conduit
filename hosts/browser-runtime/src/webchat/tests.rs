use super::{BrowserChatEffect, BrowserChatSession};
use conduit_core::{BootId, HostId};

fn session() -> BrowserChatSession {
    BrowserChatSession::prepare(
        "ws://127.0.0.1:4178",
        HostId::from("browser/test-host"),
        BootId::from("browser/test-boot"),
    )
    .unwrap()
}

fn connect(
    session: &mut BrowserChatSession,
) -> (
    conduit_presentation::Presentation,
    conduit_presentation::Manifestation,
) {
    session
        .complete_simple(BrowserChatEffect::SocketOpen)
        .unwrap();
    let mut presentation = None;
    let mut manifestation = None;
    while session.effect() == BrowserChatEffect::Present {
        presentation = Some(serde_json::from_slice(session.effect_bytes()).unwrap());
        session.complete_simple(BrowserChatEffect::Present).unwrap();
        manifestation = Some(serde_json::from_slice(session.interaction_text()).unwrap());
    }
    assert_eq!(session.effect(), BrowserChatEffect::SocketReceive);
    (presentation.unwrap(), manifestation.unwrap())
}

fn interaction_frame(
    presentation: &conduit_presentation::Presentation,
    manifestation: &conduit_presentation::Manifestation,
    value: &str,
    sequence: u64,
) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "presentation_id": presentation.identity.as_str(),
        "presentation_revision": presentation.revision,
        "manifestation_id": manifestation.manifestation_id.as_str(),
        "input_id": conduit_chat::CHAT_MESSAGE_INPUT,
        "action_id": conduit_chat::CHAT_SEND_ACTION,
        "target": conduit_chat::CHAT_MESSAGE_TARGET,
        "value_kind": conduit_presentation::UTF8_TEXT_VALUE_KIND,
        "sequence": sequence,
        "value": value,
    }))
    .unwrap()
}

#[test]
fn browser_chat_runs_planned_kernel_effects_with_preemption_and_disconnect() {
    let mut session = session();
    assert_eq!(session.effect(), BrowserChatEffect::SocketOpen);
    let (presentation, manifestation) = connect(&mut session);

    let frame = interaction_frame(&presentation, &manifestation, "hello from A", 0);
    session.submit(&frame).unwrap();
    while session.effect() == BrowserChatEffect::Present {
        session.complete_simple(BrowserChatEffect::Present).unwrap();
    }
    assert_eq!(session.effect(), BrowserChatEffect::SocketSend);
    assert_eq!(session.effect_bytes(), b"hello from A");
    session
        .complete_simple(BrowserChatEffect::SocketSend)
        .unwrap();
    assert_eq!(session.effect(), BrowserChatEffect::SocketReceive);

    session.receive(b"hello from A").unwrap();
    assert_eq!(session.effect(), BrowserChatEffect::Present);
    assert!(session
        .effect_bytes()
        .windows(12)
        .any(|bytes| bytes == b"hello from A"));
    session.complete_simple(BrowserChatEffect::Present).unwrap();
    assert_eq!(session.effect(), BrowserChatEffect::SocketReceive);
    assert!(session.capacity_stable());
    assert!(!session
        .identity_text()
        .windows(5)
        .any(|item| item == b"hello"));

    session.disconnect().unwrap();
    while session.effect() == BrowserChatEffect::Present {
        session.complete_simple(BrowserChatEffect::Present).unwrap();
    }
    assert_eq!(session.status(), 1);
    assert!(session.disconnected());
    assert!(session.request_count() >= 5);
    assert!(session.capacity_stable());
}

#[test]
fn malformed_and_oversize_browser_messages_fail_before_kernel_admission() {
    let mut session = session();
    let _ = connect(&mut session);
    assert!(session.submit(&[]).is_err());
    assert!(session
        .receive(&vec![
            0;
            conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES
                as usize
                + 1
        ])
        .is_err());
    assert_eq!(session.effect(), BrowserChatEffect::SocketReceive);
}

#[test]
fn configured_host_and_boot_are_canonical_runtime_identity() {
    let session = BrowserChatSession::prepare(
        "ws://127.0.0.1:4178",
        HostId::from("browser/independent-tab"),
        BootId::from("browser/fresh-boot"),
    )
    .unwrap();
    let identity = std::str::from_utf8(session.identity_text()).unwrap();
    assert!(identity.contains("host=browser/independent-tab"));
    assert!(identity.contains("boot=browser/fresh-boot"));
}
