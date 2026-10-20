//! Exact session bytes and lifecycle against the former cloned-binding path.
use super::*;
use conduit_wire::{encode_session_frame_into, SessionTerminalDisposition};
fn session() -> RemoteCordSession {
    let exact = conduit_signal_conformance::exact_distributed_signal_plan().unwrap();
    let fragment = exact
        .plan
        .fragments
        .iter()
        .find(|f| f.host_id == exact.source_advertisement.host_id)
        .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    RemoteCordSessions::prepare(fragment, &lowered)
        .unwrap()
        .sessions
        .remove(0)
}
fn outbound(
    session: &mut RemoteCordSession,
    reference: &mut SessionMachine,
    message: Option<SessionMessage<'_>>,
    capacity: usize,
) -> Result<usize, WireError> {
    let mut expected = [0; 2048];
    let mut actual = [0; 2048];
    let guard = crate::allocation_probe::begin();
    let binding = session.binding().clone();
    let frame = message.map_or_else(|| binding.hello_frame(), |message| binding.frame(message));
    let result = reference.admit_outbound(frame).and_then(|()| {
        encode_session_frame_into(
            frame,
            &mut expected[..capacity],
            binding.limits.maximum_payload_bytes,
            binding.attachment.limits.maximum_frame_bytes,
        )
    });
    let cloned_allocations = guard.finish();
    assert!(
        cloned_allocations > 0,
        "reference must measure actual binding cloning"
    );
    let guard = crate::allocation_probe::begin();
    let encoded = match message {
        Some(message) => session.encode_outbound(message, &mut actual[..capacity]),
        None => session.encode_outbound_hello(&mut actual[..capacity]),
    };
    let borrowed_allocations = guard.finish();
    eprintln!("remote Cord encode: cloned-binding allocations={cloned_allocations}, borrowed-binding allocations={borrowed_allocations}, result={encoded:?}");
    assert_eq!(borrowed_allocations, 0);
    assert_eq!(encoded, result);
    assert_eq!(actual, expected);
    assert_eq!(session.machine().checkpoint(), reference.checkpoint());
    assert_eq!(session.machine().is_active(), reference.is_active());
    assert_eq!(session.machine().is_terminal(), reference.is_terminal());
    result
}
fn inbound(
    session: &mut RemoteCordSession,
    reference: &mut SessionMachine,
    message: SessionMessage<'_>,
) -> Result<(), WireError> {
    let binding = session.binding().clone();
    let frame = binding.frame(message);
    let expected = reference.admit_inbound(frame);
    assert_eq!(session.machine_mut().admit_inbound(frame), expected);
    assert_eq!(session.machine().checkpoint(), reference.checkpoint());
    expected
}
fn active() -> (RemoteCordSession, SessionMachine) {
    let mut session = session();
    let mut reference =
        SessionMachine::new(session.binding().clone(), SessionRole::Source).unwrap();
    outbound(&mut session, &mut reference, None, 2048).unwrap();
    let binding = session.binding().clone();
    inbound(&mut session, &mut reference, binding.hello_frame().message).unwrap();
    outbound(
        &mut session,
        &mut reference,
        Some(SessionMessage::Ready),
        2048,
    )
    .unwrap();
    inbound(&mut session, &mut reference, SessionMessage::Ready).unwrap();
    assert!(session.machine().is_active());
    (session, reference)
}
#[test]
fn borrowed_codec_is_allocation_free_and_preserves_acceptance_delivery_and_terminal_bytes() {
    let (mut session, mut reference) = active();
    let payload = [1; 8];
    outbound(
        &mut session,
        &mut reference,
        Some(SessionMessage::Offered {
            sequence: 0,
            payload: &payload,
        }),
        2048,
    )
    .unwrap();
    assert!(inbound(
        &mut session,
        &mut reference,
        SessionMessage::Delivered { sequence: 0 }
    )
    .is_err());
    inbound(
        &mut session,
        &mut reference,
        SessionMessage::Accepted { sequence: 0 },
    )
    .unwrap();
    assert_eq!(
        session.machine().next_sequence(),
        0,
        "acceptance is not delivery"
    );
    inbound(
        &mut session,
        &mut reference,
        SessionMessage::Delivered { sequence: 0 },
    )
    .unwrap();
    assert_eq!(session.machine().next_sequence(), 1);
    outbound(
        &mut session,
        &mut reference,
        Some(SessionMessage::InputClosed { final_sequence: 1 }),
        2048,
    )
    .unwrap();
    let terminal = SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Completed,
        final_sequence: 1,
    };
    outbound(&mut session, &mut reference, Some(terminal), 2048).unwrap();
    inbound(&mut session, &mut reference, terminal).unwrap();
    assert!(session.machine().is_terminal());
}
#[test]
fn borrowed_codec_preserves_refusals_cancellation_and_admission_before_encoding_failure() {
    let mut session = session();
    let mut reference =
        SessionMachine::new(session.binding().clone(), SessionRole::Source).unwrap();
    assert!(outbound(&mut session, &mut reference, None, 1).is_err());
    // Both paths retain the same already-admitted Hello transition after failure.
    assert_eq!(
        outbound(&mut session, &mut reference, None, 2048),
        Err(WireError::DuplicateFrame)
    );
    let (mut session, mut reference) = active();
    let payload = vec![0; session.binding().limits.maximum_payload_bytes as usize + 1];
    assert!(outbound(
        &mut session,
        &mut reference,
        Some(SessionMessage::Offered {
            sequence: 0,
            payload: &payload
        }),
        2048
    )
    .is_err());
    outbound(
        &mut session,
        &mut reference,
        Some(SessionMessage::Cancelled { code: 9 }),
        2048,
    )
    .unwrap();
    assert!(!session.machine().is_active());
    assert!(outbound(
        &mut session,
        &mut reference,
        Some(SessionMessage::Offered {
            sequence: 0,
            payload: &[1; 8]
        }),
        2048
    )
    .is_err());
}
