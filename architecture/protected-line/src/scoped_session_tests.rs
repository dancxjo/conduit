//! Actual handshake/crypto plus inspection of the SAME private admission pool.
use super::*;
use crate::ProtectedSessionPolicy;
use crate::{CarrierFailure, EndpointBinding, ProtectedHandshake};
use core::{
    future::{poll_fn, Future},
    pin::pin,
    task::{Context, Poll, Waker},
};
fn binding() -> SessionBinding {
    SessionBinding {
        initiator: EndpointBinding {
            host_id: "a".into(),
            boot_id: "a/boot".into(),
        },
        responder: EndpointBinding {
            host_id: "b".into(),
            boot_id: "b/boot".into(),
        },
        negotiation_id: "negotiation".into(),
        line_session_id: "line".into(),
        candidate_binding: "candidate".into(),
        transport_binding: "transport".into(),
    }
}
fn policy() -> ProtectedSessionPolicy {
    ProtectedSessionPolicy {
        traffic: SessionLimits {
            maximum_payload_bytes: 64,
            maximum_frames_per_direction: 8,
            maximum_bytes_per_direction: 128,
        },
        maximum_simultaneous_sessions: 2,
        maximum_pending_frames_per_session: 1,
        handshake_work_units: 2,
        handshake_timeout_millis: 1000,
        idle_timeout_millis: 1000,
    }
}
fn drive<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..8 {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
    }
    panic!("bounded test future did not finish")
}
struct Peer {
    handshake: Option<ProtectedHandshake>,
    reply: [u8; 64],
    length: usize,
    calls: usize,
    failure: Option<CarrierFailure>,
    pending: bool,
    corrupt: bool,
}
impl Peer {
    fn new(binding: &SessionBinding) -> Self {
        Self {
            handshake: Some(
                ProtectedHandshake::new(
                    Role::Responder,
                    binding,
                    policy().traffic,
                    [3; 32],
                    [2; 32],
                )
                .unwrap(),
            ),
            reply: [0; 64],
            length: 0,
            calls: 0,
            failure: None,
            pending: false,
            corrupt: false,
        }
    }
}
impl AsyncProtectedFrameCarrier for Peer {
    async fn send_frame(&mut self, frame: &[u8]) -> Result<(), CarrierFailure> {
        self.calls += 1;
        if self.pending {
            poll_fn(|_| Poll::<()>::Pending).await;
        }
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        let peer = self.handshake.as_mut().unwrap();
        peer.read_message(frame).unwrap();
        self.length = peer.next_message_bytes().unwrap();
        peer.write_message(&mut self.reply[..self.length]).unwrap();
        Ok(())
    }
    async fn receive_frame(
        &mut self,
        output: &mut [u8],
        timeout: u32,
    ) -> Result<usize, CarrierFailure> {
        self.calls += 1;
        assert_eq!(timeout, policy().handshake_timeout_millis);
        output[..self.length].copy_from_slice(&self.reply[..self.length]);
        if self.corrupt {
            output[self.length - 1] ^= 1;
        }
        Ok(self.length)
    }
}
#[test]
fn invalid_policy_and_original_capacity_refuse_without_io() {
    let mut invalid = policy();
    invalid.handshake_work_units = 1;
    let binding = binding();
    let peer = Peer::new(&binding);
    assert!(matches!(
        ProtectedSessionAdmission::<2>::new(invalid),
        Err(ProtectedLineError::InvalidPolicy)
    ));
    let mut pool = ProtectedSessionAdmission::<2>::new(policy()).unwrap();
    let first = pool.admit().unwrap();
    let second = pool.admit().unwrap();
    assert!(matches!(
        pool.scoped_attempt(),
        Err(ProtectedLineError::SessionCapacity)
    ));
    assert_eq!(peer.calls, 0);
    assert_eq!(pool.occupied, [true, true]);
    pool.retire(first).unwrap();
    pool.retire(second).unwrap();
}
#[test]
fn failed_handshake_releases_only_its_original_slot() {
    let binding = binding();
    let mut pool = ProtectedSessionAdmission::<2>::new(policy()).unwrap();
    let held = pool.admit().unwrap();
    for failure in [
        CarrierFailure::Cancelled,
        CarrierFailure::Lost,
        CarrierFailure::Pressure,
    ] {
        let mut peer = Peer::new(&binding);
        peer.failure = Some(failure);
        let attempt = pool.scoped_attempt().unwrap();
        assert_eq!(attempt.slot.index, 1);
        assert_eq!(attempt.slot.pool.occupied, [true, true]);
        assert!(drive(attempt.establish_async(
            &mut peer,
            Role::Initiator,
            &binding,
            [3; 32],
            [1; 32]
        ))
        .is_err());
        assert_eq!(peer.calls, 1);
        assert_eq!(pool.occupied, [true, false]);
    }
    let mut peer = Peer::new(&binding);
    peer.corrupt = true;
    assert!(matches!(
        drive(pool.scoped_attempt().unwrap().establish_async(
            &mut peer,
            Role::Initiator,
            &binding,
            [3; 32],
            [1; 32]
        )),
        Err(ProtectedLineError::AuthenticationFailed)
    ));
    assert_eq!(peer.calls, 2);
    assert_eq!(pool.occupied, [true, false]);
    // A crypto setup refusal also unwinds original occupancy without carrier IO.
    let mut peer = Peer::new(&binding);
    assert!(drive(pool.scoped_attempt().unwrap().establish_async(
        &mut peer,
        Role::Initiator,
        &binding,
        [0; 32],
        [1; 32]
    ))
    .is_err());
    assert_eq!(peer.calls, 0);
    assert_eq!(pool.occupied, [true, false]);
    pool.retire(held).unwrap();
}
#[test]
fn dropping_attempt_and_unpolled_or_pending_handshake_releases_capacity() {
    let binding = binding();
    let mut pool = ProtectedSessionAdmission::<2>::new(policy()).unwrap();
    let held = pool.admit().unwrap();
    drop(pool.scoped_attempt().unwrap());
    assert_eq!(pool.occupied, [true, false]);
    let mut peer = Peer::new(&binding);
    let future = pool.scoped_attempt().unwrap().establish_async(
        &mut peer,
        Role::Initiator,
        &binding,
        [3; 32],
        [1; 32],
    );
    drop(future);
    assert_eq!(peer.calls, 0);
    assert_eq!(pool.occupied, [true, false]);
    peer.pending = true;
    {
        let mut future = pin!(pool.scoped_attempt().unwrap().establish_async(
            &mut peer,
            Role::Initiator,
            &binding,
            [3; 32],
            [1; 32]
        ));
        assert!(future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
    }
    assert_eq!(peer.calls, 1);
    assert_eq!(pool.occupied, [true, false]);
    assert_eq!(pool.admit().unwrap(), 1);
    pool.retire(1).unwrap();
    pool.retire(held).unwrap();
}
#[test]
fn success_keeps_same_slot_through_crypto_then_drop_close_or_cancel_permits_next_attempt() {
    let binding = binding();
    let mut pool = ProtectedSessionAdmission::<2>::new(policy()).unwrap();
    let held = pool.admit().unwrap();
    for retirement in 0..3 {
        let mut peer = Peer::new(&binding);
        let attempt = pool.scoped_attempt().unwrap();
        assert_eq!(attempt.slot.index, 1);
        let mut session =
            drive(attempt.establish_async(&mut peer, Role::Initiator, &binding, [3; 32], [1; 32]))
                .unwrap();
        let mut peer = peer.handshake.take().unwrap().finish().unwrap();
        assert_eq!(session.slot.as_ref().unwrap().index, 1);
        assert_eq!(session.slot.as_ref().unwrap().pool.occupied, [true, true]);
        assert_eq!(session.evidence().binding, &binding);
        assert_eq!(session.limits(), policy().traffic);
        let mut encrypted = [0; 128];
        let mut plain = [0; 64];
        assert_eq!(
            session.seal(b"request", &mut []),
            Err(ProtectedLineError::OutputTooSmall)
        );
        assert!(
            session.slot.is_some(),
            "local buffer pressure does not retire valid crypto"
        );
        let length = session.seal(b"request", &mut encrypted).unwrap();
        let length = peer.open(&encrypted[..length], &mut plain).unwrap();
        assert_eq!(&plain[..length], b"request");
        let length = peer.seal(b"result", &mut encrypted).unwrap();
        let length = session.open(&encrypted[..length], &mut plain).unwrap();
        assert_eq!(&plain[..length], b"result");
        assert_eq!(session.slot.as_ref().unwrap().pool.occupied, [true, true]);
        match retirement {
            0 => drop(session),
            1 => session.close(),
            _ => session.cancel(),
        }
        assert_eq!(pool.occupied, [true, false]);
    }
    pool.retire(held).unwrap();
    assert_eq!(pool.occupied, [false, false]);
}
#[test]
fn terminal_crypto_failure_releases_slot_once_before_wrapper_drop() {
    let binding = binding();
    let mut pool = ProtectedSessionAdmission::<2>::new(policy()).unwrap();
    let mut peer = Peer::new(&binding);
    let mut session = drive(pool.scoped_attempt().unwrap().establish_async(
        &mut peer,
        Role::Initiator,
        &binding,
        [3; 32],
        [1; 32],
    ))
    .unwrap();
    assert!(session.open(b"malformed", &mut [0; 64]).is_err());
    assert!(matches!(
        session.evidence().disposition,
        SessionDisposition::Failed(_)
    ));
    assert!(session.slot.is_none());
    assert_eq!(
        session.open(b"malformed", &mut [0; 64]),
        Err(ProtectedLineError::Closed)
    );
    drop(session);
    assert_eq!(pool.occupied, [false, false]);
    assert_eq!(pool.admit().unwrap(), 0);
    pool.retire(0).unwrap();
}
