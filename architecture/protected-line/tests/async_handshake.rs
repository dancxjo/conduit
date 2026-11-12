#![cfg(feature = "noise-session")]
use conduit_protected_line::*;
use std::{
    cell::RefCell,
    future::{poll_fn, Future},
    pin::pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
fn binding() -> SessionBinding {
    SessionBinding {
        initiator: EndpointBinding {
            host_id: "host/a".into(),
            boot_id: "boot/a".into(),
        },
        responder: EndpointBinding {
            host_id: "host/b".into(),
            boot_id: "boot/b".into(),
        },
        negotiation_id: "negotiation/1".into(),
        line_session_id: "line/1".into(),
        candidate_binding: "candidate/1".into(),
        transport_binding: "usb/1".into(),
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
#[derive(Default)]
struct Pipe {
    frame: Option<Vec<u8>>,
    waker: Option<Waker>,
}
struct Carrier {
    send: Rc<RefCell<Pipe>>,
    receive: Rc<RefCell<Pipe>>,
    calls: usize,
    failure: Option<CarrierFailure>,
    oversized: bool,
}
fn pair() -> (Carrier, Carrier) {
    let a = Rc::new(RefCell::new(Pipe::default()));
    let b = Rc::new(RefCell::new(Pipe::default()));
    let make = |send, receive| Carrier {
        send,
        receive,
        calls: 0,
        failure: None,
        oversized: false,
    };
    (make(a.clone(), b.clone()), make(b, a))
}
async fn yield_once() {
    let mut yielded = false;
    poll_fn(|cx| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}
impl AsyncProtectedFrameCarrier for Carrier {
    async fn send_frame(&mut self, frame: &[u8]) -> Result<(), CarrierFailure> {
        self.calls += 1;
        yield_once().await;
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        let wake = {
            let mut pipe = self.send.borrow_mut();
            if pipe.frame.is_some() {
                return Err(CarrierFailure::Pressure);
            }
            pipe.frame = Some(frame.to_vec());
            pipe.waker.take()
        };
        if let Some(waker) = wake {
            waker.wake();
        }
        Ok(())
    }
    async fn receive_frame(
        &mut self,
        output: &mut [u8],
        timeout: u32,
    ) -> Result<usize, CarrierFailure> {
        assert_eq!(timeout, policy().handshake_timeout_millis);
        self.calls += 1;
        yield_once().await;
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if self.oversized {
            return Ok(output.len() + 1);
        }
        poll_fn(|cx| {
            let mut pipe = self.receive.borrow_mut();
            if let Some(frame) = pipe.frame.take() {
                if frame.len() > output.len() {
                    return Poll::Ready(Err(CarrierFailure::Pressure));
                }
                output[..frame.len()].copy_from_slice(&frame);
                Poll::Ready(Ok(frame.len()))
            } else {
                pipe.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
}
fn drive<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..32 {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
    }
    panic!("bounded test future did not finish")
}
fn handshake(
    a: &mut Carrier,
    b: &mut Carrier,
    left: &SessionBinding,
    right: &SessionBinding,
    right_key: [u8; 32],
) -> (
    Option<Result<ProtectedSession, ProtectedLineError>>,
    Result<ProtectedSession, ProtectedLineError>,
    usize,
) {
    let mut initiator = pin!(establish_protected_session_async(
        a,
        Role::Initiator,
        left,
        policy(),
        [3; 32],
        [1; 32]
    ));
    let mut responder = pin!(establish_protected_session_async(
        b,
        Role::Responder,
        right,
        policy(),
        right_key,
        [2; 32]
    ));
    let mut first = None;
    let mut second = None;
    let unrelated = std::cell::Cell::new(0);
    let mut actor = pin!(poll_fn(|_| {
        unrelated.set(unrelated.get() + 1);
        Poll::<()>::Pending
    }));
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..32 {
        if first.is_none() {
            if let Poll::Ready(result) = initiator.as_mut().poll(&mut cx) {
                first = Some(result);
            }
        }
        if second.is_none() {
            if let Poll::Ready(result) = responder.as_mut().poll(&mut cx) {
                second = Some(result);
            }
        }
        // A separate actor progresses between handshake polls, including pending IO.
        assert!(actor.as_mut().poll(&mut cx).is_pending());
        if second.as_ref().is_some_and(Result::is_err) {
            return (first, second.unwrap(), unrelated.get());
        }
        if first.is_some() {
            if let Some(result) = second.take() {
                return (first, result, unrelated.get());
            }
        }
    }
    panic!("handshake did not settle")
}
#[test]
fn yielding_two_endpoint_handshake_preserves_binding_limits_and_encrypted_traffic() {
    let (mut a, mut b) = pair();
    let binding = binding();
    let (a, b, progress) = handshake(&mut a, &mut b, &binding, &binding, [3; 32]);
    let mut a = a.unwrap().unwrap();
    let mut b = b.unwrap();
    assert!(progress > 1);
    assert_eq!(a.evidence().binding, &binding);
    assert_eq!(b.evidence().binding, &binding);
    assert_eq!(a.limits(), policy().traffic);
    assert_eq!(b.limits(), policy().traffic);
    let mut encrypted = [0; 128];
    let mut plain = [0; 64];
    for message in [b"request".as_slice(), b"result".as_slice()] {
        let length = a.seal(message, &mut encrypted).unwrap();
        let length = b.open(&encrypted[..length], &mut plain).unwrap();
        assert_eq!(&plain[..length], message);
        let length = b.seal(message, &mut encrypted).unwrap();
        let length = a.open(&encrypted[..length], &mut plain).unwrap();
        assert_eq!(&plain[..length], message);
    }
}
#[test]
fn invalid_policy_refuses_before_io_and_failure_mapping_matches_sync_helper() {
    let binding = binding();
    let (mut carrier, _) = pair();
    let mut invalid = policy();
    invalid.handshake_work_units = 1;
    assert!(matches!(
        drive(establish_protected_session_async(
            &mut carrier,
            Role::Initiator,
            &binding,
            invalid,
            [3; 32],
            [1; 32]
        )),
        Err(ProtectedLineError::InvalidPolicy)
    ));
    assert_eq!(carrier.calls, 0);
    for (failure, expected) in [
        (CarrierFailure::Cancelled, ProtectedLineError::Cancelled),
        (CarrierFailure::Lost, ProtectedLineError::OuterCarrierLost),
        (CarrierFailure::Pressure, ProtectedLineError::Pressure),
        (
            CarrierFailure::TimedOut,
            ProtectedLineError::HandshakeTimedOut,
        ),
        (CarrierFailure::Closed, ProtectedLineError::Closed),
    ] {
        for role in [Role::Initiator, Role::Responder] {
            let (mut carrier, _) = pair();
            carrier.failure = Some(failure);
            assert!(
                matches!(drive(establish_protected_session_async(&mut carrier, role, &binding, policy(), [3;32], [1;32])), Err(error) if error == expected)
            );
            assert_eq!(carrier.calls, 1);
        }
    }
    let (mut carrier, _) = pair();
    carrier.oversized = true;
    assert!(matches!(
        drive(establish_protected_session_async(
            &mut carrier,
            Role::Responder,
            &binding,
            policy(),
            [3; 32],
            [1; 32]
        )),
        Err(ProtectedLineError::Pressure)
    ));
}
#[test]
fn wrong_key_or_binding_cannot_authenticate_the_responder() {
    let original = binding();
    for changed_key in [false, true] {
        let mut other = binding();
        if !changed_key {
            other.responder.boot_id = "wrong/boot".into();
        }
        let (mut a, mut b) = pair();
        let (initiator, responder, _) = handshake(
            &mut a,
            &mut b,
            &original,
            &other,
            if changed_key { [9; 32] } else { [3; 32] },
        );
        assert!(!matches!(initiator, Some(Ok(_))));
        assert!(matches!(
            responder,
            Err(ProtectedLineError::AuthenticationFailed)
        ));
    }
}

// Only the original synchronous peer uses this adapter, after the initiator has
// supplied its message. The async helper has no synchronous carrier trait bound.
struct SyncPeer<'a>(&'a mut Carrier);
impl ProtectedFrameCarrier for SyncPeer<'_> {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), CarrierFailure> {
        let mut pipe = self.0.send.borrow_mut();
        if pipe.frame.is_some() {
            return Err(CarrierFailure::Pressure);
        }
        pipe.frame = Some(frame.to_vec());
        if let Some(waker) = pipe.waker.take() {
            waker.wake();
        }
        Ok(())
    }
    fn receive_frame(&mut self, output: &mut [u8], timeout: u32) -> Result<usize, CarrierFailure> {
        assert_eq!(timeout, policy().handshake_timeout_millis);
        let frame = self
            .0
            .receive
            .borrow_mut()
            .frame
            .take()
            .ok_or(CarrierFailure::Pressure)?;
        if frame.len() > output.len() {
            return Err(CarrierFailure::Pressure);
        }
        output[..frame.len()].copy_from_slice(&frame);
        Ok(frame.len())
    }
    fn close(&mut self) -> Result<(), CarrierFailure> {
        Ok(())
    }
}
#[test]
fn asynchronous_initiator_interoperates_with_original_synchronous_responder() {
    let (mut a, mut b) = pair();
    let binding = binding();
    let mut initiator = pin!(establish_protected_session_async(
        &mut a,
        Role::Initiator,
        &binding,
        policy(),
        [3; 32],
        [1; 32]
    ));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(initiator.as_mut().poll(&mut cx).is_pending());
    assert!(initiator.as_mut().poll(&mut cx).is_pending());
    let mut responder = establish_protected_session(
        &mut SyncPeer(&mut b),
        Role::Responder,
        &binding,
        policy(),
        [3; 32],
        [2; 32],
    )
    .unwrap();
    let mut initiator = drive(initiator).unwrap();
    let mut encrypted = [0; 128];
    let mut plain = [0; 64];
    let length = initiator.seal(b"request", &mut encrypted).unwrap();
    let length = responder.open(&encrypted[..length], &mut plain).unwrap();
    assert_eq!(&plain[..length], b"request");
    let length = responder.seal(b"result", &mut encrypted).unwrap();
    let length = initiator.open(&encrypted[..length], &mut plain).unwrap();
    assert_eq!(&plain[..length], b"result");
}
