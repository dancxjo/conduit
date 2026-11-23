//! Actual selected-session state and encoded responses, with synthetic model
//! terminal inputs. No provider inference or physical transport is invoked.
use super::*;
use conduit_wire::{
    decode_session_frame, SessionBinding, SessionMachine, SessionRole, SessionTerminalDisposition,
};
mod planned_sessions;

struct Peer {
    binding: SessionBinding,
    machine: SessionMachine,
}
impl Peer {
    fn receive(&mut self, bytes: &[u8]) {
        let frame = decode_session_frame(
            bytes,
            self.binding.limits.maximum_payload_bytes,
            self.binding.attachment.limits.maximum_frame_bytes,
        )
        .unwrap();
        assert_eq!(frame.identity, self.binding.identity());
        self.machine.admit_inbound(frame).unwrap();
    }
}

fn ready_operation() -> (PoolMemberSessions, Vec<Peer>) {
    let mut sessions = planned_sessions::sessions();
    let hellos = encode_pool_member_hello_frames(&mut sessions).unwrap();
    let mut peers = Vec::new();
    for (session, hello) in sessions.iter_mut().zip(hellos) {
        let binding = session.binding().clone();
        let role = match session.direction {
            PoolMemberSessionDirection::Input => SessionRole::Source,
            PoolMemberSessionDirection::Output => SessionRole::Sink,
        };
        let mut peer = Peer {
            machine: SessionMachine::new(binding.clone(), role).unwrap(),
            binding: binding.clone(),
        };
        peer.receive(&hello);
        let hello = binding.hello_frame();
        peer.machine.admit_outbound(hello).unwrap();
        session.machine_mut().admit_inbound(hello).unwrap();
        let ready = binding.frame(SessionMessage::Ready);
        peer.machine.admit_outbound(ready).unwrap();
        session.machine_mut().admit_inbound(ready).unwrap();
        peer.receive(&pool_member_response(session, SessionMessage::Ready).unwrap());
        assert!(session.machine().is_active());
        assert!(peer.machine.is_active());
        if session.direction == PoolMemberSessionDirection::Input {
            let offered = binding.frame(SessionMessage::Offered {
                sequence: 0,
                payload: b"request",
            });
            peer.machine.admit_outbound(offered).unwrap();
            session.machine_mut().admit_inbound(offered).unwrap();
            for message in [
                SessionMessage::Accepted { sequence: 0 },
                SessionMessage::Delivered { sequence: 0 },
            ] {
                peer.receive(&pool_member_response(session, message).unwrap());
            }
            assert_eq!(session.machine().next_sequence(), 1);
        }
        peers.push(peer);
    }
    (sessions, peers)
}

fn collect_operation(
    sessions: &mut PoolMemberSessions,
    terminal: LocalModelAdapterTerminal,
    output: &[u8],
) -> Result<Vec<Vec<u8>>, String> {
    let mut responses = Vec::new();
    complete_pool_member_operation(sessions, terminal, output, &mut responses)?;
    Ok(responses)
}

fn assert_terminal(
    terminal: LocalModelAdapterTerminal,
    code: u16,
    disposition: SessionTerminalDisposition,
) {
    let (mut sessions, mut peers) = ready_operation();
    // A failed adapter may leave bytes. Even bytes beyond the semantic payload
    // ceiling must not turn timeout/wire overflow into payload failure or output.
    let responses = collect_operation(&mut sessions, terminal, &[b'x'; 65]).unwrap();
    assert_eq!(responses.len(), 4);
    let (pairs, remainder) = responses.as_chunks::<2>();
    assert!(remainder.is_empty());
    for (frames, peer) in pairs.iter().zip(&mut peers) {
        let decode =
            |index: usize| decode_session_frame(frames[index].as_slice(), 64, 4_096).unwrap();
        let failure = decode(0);
        assert_eq!(failure.identity, peer.binding.identity());
        match disposition {
            SessionTerminalDisposition::Cancelled => {
                assert_eq!(failure.message, SessionMessage::Cancelled { code })
            }
            _ => assert_eq!(failure.message, SessionMessage::Failed { code }),
        }
        peer.receive(&frames[0]);
        let terminal_frame = decode(1);
        assert_eq!(
            terminal_frame.message,
            SessionMessage::Terminal {
                disposition,
                final_sequence: peer.machine.next_sequence(),
            }
        );
        peer.receive(&frames[1]);
        assert!(!peer.machine.is_active());
        let reply = peer.binding.frame(SessionMessage::Terminal {
            disposition,
            final_sequence: peer.machine.next_sequence(),
        });
        peer.machine.admit_outbound(reply).unwrap();
        sessions
            .session_for_binding_mut(&reply.identity)
            .unwrap()
            .machine_mut()
            .admit_inbound(reply)
            .unwrap();
        assert!(peer.machine.is_terminal());
    }
    assert!(sessions
        .iter()
        .all(|session| session.machine().is_terminal()));
    if let Ok(responses) = collect_operation(&mut sessions, terminal, b"late output") {
        assert!(responses.is_empty());
    }
    for session in sessions.iter_mut() {
        let binding = session.binding().clone();
        let stale = binding.frame(SessionMessage::Offered {
            sequence: session.machine().next_sequence(),
            payload: b"late output",
        });
        assert!(session.machine_mut().admit_outbound(stale).is_err());
    }
}

#[test]
fn timeout_and_raw_response_bound_retire_both_sessions_with_distinct_failures_and_no_payload() {
    assert_terminal(
        LocalModelAdapterTerminal::TimedOut,
        75,
        SessionTerminalDisposition::Failed,
    );
    assert_terminal(
        LocalModelAdapterTerminal::ResponseBoundExceeded,
        76,
        SessionTerminalDisposition::Failed,
    );
}

#[test]
fn existing_failure_and_cancel_outcomes_keep_their_session_codes_and_dispositions() {
    for (terminal, code) in [
        (LocalModelAdapterTerminal::Refused, 70),
        (LocalModelAdapterTerminal::Failed, 71),
        (LocalModelAdapterTerminal::InvalidStructuredResult, 71),
        (LocalModelAdapterTerminal::ProviderLost, 74),
    ] {
        assert_terminal(terminal, code, SessionTerminalDisposition::Failed);
    }
    assert_terminal(
        LocalModelAdapterTerminal::Cancelled,
        72,
        SessionTerminalDisposition::Cancelled,
    );
}

#[test]
fn produced_and_truncated_keep_output_publication_and_semantic_payload_bound() {
    for terminal in [
        LocalModelAdapterTerminal::Produced,
        LocalModelAdapterTerminal::Truncated,
    ] {
        let (mut sessions, mut peers) = ready_operation();
        let responses = collect_operation(&mut sessions, terminal, b"bounded result").unwrap();
        assert_eq!(responses.len(), 1);
        let output = sessions
            .iter()
            .find(|s| s.direction == PoolMemberSessionDirection::Output)
            .unwrap();
        let frame = decode_session_frame(&responses[0], 64, 4_096).unwrap();
        assert_eq!(frame.identity, output.binding().identity());
        assert_eq!(
            frame.message,
            SessionMessage::Offered {
                sequence: 0,
                payload: b"bounded result"
            }
        );
        peers
            .iter_mut()
            .find(|peer| peer.binding.identity() == frame.identity)
            .unwrap()
            .receive(&responses[0]);
        assert!(sessions.iter().all(|session| session.machine().is_active()));
        assert_terminal(terminal, 73, SessionTerminalDisposition::Failed);
    }
}
