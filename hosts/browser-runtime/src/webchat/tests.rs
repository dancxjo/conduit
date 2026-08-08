use super::{BrowserChatEffect, BrowserChatSession};

#[test]
fn browser_chat_runs_planned_kernel_effects_with_preemption_and_disconnect() {
    let mut session = BrowserChatSession::prepare("ws://127.0.0.1:4178").unwrap();
    assert_eq!(session.effect(), BrowserChatEffect::SocketOpen);
    session
        .complete_simple(BrowserChatEffect::SocketOpen)
        .unwrap();
    assert_eq!(session.effect(), BrowserChatEffect::SocketReceive);

    session.submit(b"hello from A").unwrap();
    assert_eq!(session.effect(), BrowserChatEffect::SocketSend);
    assert_eq!(session.effect_bytes(), b"hello from A");
    session
        .complete_simple(BrowserChatEffect::SocketSend)
        .unwrap();
    assert_eq!(session.effect(), BrowserChatEffect::SocketReceive);

    session.receive(b"hello from A").unwrap();
    assert_eq!(session.effect(), BrowserChatEffect::ListAppend);
    assert_eq!(session.effect_bytes(), b"hello from A");
    session
        .complete_simple(BrowserChatEffect::ListAppend)
        .unwrap();
    assert_eq!(session.effect(), BrowserChatEffect::SocketReceive);
    assert!(session.capacity_stable());
    assert!(!session
        .identity_text()
        .windows(5)
        .any(|item| item == b"hello"));

    session.disconnect().unwrap();
    assert_eq!(session.status(), 1);
    assert!(session.disconnected());
    assert!(session.request_count() >= 5);
    assert!(session.capacity_stable());
}

#[test]
fn malformed_and_oversize_browser_messages_fail_before_kernel_admission() {
    let mut session = BrowserChatSession::prepare("ws://127.0.0.1:4178").unwrap();
    session
        .complete_simple(BrowserChatEffect::SocketOpen)
        .unwrap();
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
