//! Framed CPU peers only; no physical/Body/kernel-start proof.
use super::*;
#[test]
fn explicit_handshake_accepts_interleaved_original_sessions_once() {
    let mut client = carriage();
    let mut output = [0; 128];
    client.handshake(&mut output, TIMEOUT).unwrap();
    assert!(client.state.machines.iter().all(Machine::is_active));
    assert_eq!(client.state.used_frames, 8);
    assert_eq!(client.frames.writes, 4);
    assert!(client.frames.queue.is_empty());
    assert_eq!(
        client.handshake(&mut output, TIMEOUT),
        Err(ForeCarriageError::Scope)
    );
    assert_eq!(client.frames.writes, 4);
    assert!(!client.state.retired);
}
#[test]
fn known_frame_and_storage_shortages_refuse_before_any_io() {
    let mut client = carriage();
    let mut output = [0; 128];
    client.state.bounds.maximum_frames = 7;
    assert_eq!(
        client.handshake(&mut output, TIMEOUT),
        Err(ForeCarriageError::Bounds)
    );
    assert_eq!(client.frames.writes, 0);
    assert_eq!(client.state.used_frames, 0);
    let mut client = carriage();
    assert_eq!(
        client.handshake(&mut output[..127], TIMEOUT),
        Err(ForeCarriageError::Bounds)
    );
    assert_eq!(client.frames.writes, 0);
    assert_eq!(
        client.handshake(&mut output, Duration::ZERO),
        Err(ForeCarriageError::Bounds)
    );
    assert_eq!(client.frames.writes, 0);
}
#[test]
fn interruption_retires_partial_handshake_without_retry_or_application_ack() {
    let mut client = carriage();
    let mut output = [0; 128];
    client.frames.fail_receive = true;
    assert_eq!(
        client.handshake(&mut output, TIMEOUT),
        Err(ForeCarriageError::OutcomeUnknown)
    );
    assert!(client.state.retired);
    assert_eq!(client.frames.writes, 2);
    assert_eq!(
        client.handshake(&mut output, TIMEOUT),
        Err(ForeCarriageError::Retired)
    );
    assert_eq!(client.frames.writes, 2);
}
#[test]
fn peer_scope_is_checked_against_original_binding() {
    let mut client = carriage();
    let mut output = [0; 128];
    let mut wire = [0; 128];
    let mut foreign = bindings()[0].clone();
    foreign.scope_commitment = [99; 32];
    let length =
        encode_external_fore_session_frame_into(foreign.frame(Message::Hello), &mut wire, 16, 128)
            .unwrap();
    client.frames.queue.push_back(wire[..length].to_vec());
    assert!(client.handshake(&mut output, TIMEOUT).is_err());
    assert!(client.state.retired);
    assert_eq!(client.frames.writes, 2);
}

#[test]
fn first_route_can_be_ready_before_second_peer_hello() {
    let mut client = carriage();
    client.frames.delay_second_hello = true;
    let mut output = [0; 128];
    client.handshake(&mut output, TIMEOUT).unwrap();
    assert!(client.state.machines.iter().all(Machine::is_active));
    assert_eq!(client.state.used_frames, 8);
    assert_eq!(client.frames.writes, 4);
}
