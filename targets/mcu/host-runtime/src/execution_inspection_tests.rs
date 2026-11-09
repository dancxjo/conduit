use super::*;
use conduit_kernel::{NodeId, RequestId};
#[test]
fn inspection_counts_only_observed_completions_and_discloses_overwrite() {
    let state = State::<1>::from_digest([7; 32]);
    let mut event = KernelEvent {
        sequence: 4,
        node: NodeId(2),
        port: None,
        request: Some(RequestId(9)),
        kind: KernelEventKind::HostCallRequested,
    };
    state.record(0, event).unwrap();
    state.record(0, event).unwrap();
    event.sequence = 5;
    event.kind = KernelEventKind::HostCallCompleted;
    state.record(0, event).unwrap();
    state.observe(0, Observation::EvidenceCapacity).unwrap();
    let mut output = [0; RESPONSE_BYTES];
    let n = state.encode(&[b'S', 0, 1], &mut output).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&output[..n]).unwrap();
    assert_eq!(report["fragments"][0]["observed_events"], 2);
    assert_eq!(report["fragments"][0]["completed_calls"], 1);
    assert_eq!(report["fragments"][0]["overwritten_slots"], 1);
    assert_eq!(report["selected"]["available"], false);
    assert_eq!(report["fragments"][0]["condition"], "evidence-capacity");
    assert_eq!(state.record(1, event), Err(Refusal::Fragment));
    event.request = None;
    assert_eq!(state.record(0, event), Err(Refusal::Event));
    assert_eq!(state.fragments[0].get().completions, 1);
    assert_eq!(state.encode(b"?", &mut [0; 8]), Err(Refusal::Capacity));
}
#[test]
fn refusal_details_are_bounded_utf8_and_preserve_original_observations() {
    struct Long;
    impl fmt::Debug for Long {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for _ in 0..40 {
                f.write_str("界")?;
            }
            Ok(())
        }
    }
    let state = State::<3>::from_digest([6; 32]);
    state.observe(1, Observation::Refused).unwrap();
    state.observe_refusal(1, "step", &Long).unwrap();
    assert_eq!(
        state.observe_refusal(3, "frame", &Long),
        Err(Refusal::Fragment)
    );
    let mut output = [0; RESPONSE_BYTES];
    let n = state.encode(b"?", &mut output).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&output[..n]).unwrap();
    let fragment = &report["fragments"][1];
    assert_eq!(fragment["condition"], "refused");
    assert_eq!(fragment["refusal"]["phase"], "step");
    assert_eq!(fragment["refusal"]["truncated"], true);
    assert_eq!(
        fragment["refusal"]["error"].as_str().unwrap().len(),
        REFUSAL_BYTES
    );
    assert_eq!(fragment["observed_events"], 0);
    assert!(report["fragments"][0]["refusal"].is_null());
    state.observe_refusal(1, "begin", &"Face").unwrap();
    let n = state.encode(b"?", &mut output).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&output[..n]).unwrap();
    assert_eq!(report["fragments"][1]["refusal"]["phase"], "begin");
    assert_eq!(report["fragments"][1]["refusal"]["error"], "\"Face\"");
    assert_eq!(report["fragments"][1]["refusal"]["truncated"], false);
    // Three retained slots remain below 400 extra inline bytes even on the
    // 64-bit test host; MCU slots are smaller. Encoding streams one slot.
    assert!(3 * core::mem::size_of::<Option<RefusalDetail>>() <= 400);
}
#[test]
fn extended_inspection_encodes_synchronously_before_send_and_refuses_overflow() {
    struct Carrier;
    impl MessageTransport for Carrier {
        type Error = ();
        async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
            output[0] = b'F';
            Ok(&output[..1])
        }
        async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
            assert_eq!(bytes, b"retained");
            Err(())
        }
    }
    let state = State::<1>::from_digest([5; 32]);
    let calls = Cell::new(0);
    let mut output = [0; 8];
    let mut carrier = Carrier;
    let mut future = core::pin::pin!(serve_with(
        &state,
        &mut carrier,
        &mut output,
        |request, bytes| {
            assert_eq!(request, b"F");
            calls.set(calls.get() + 1);
            bytes.copy_from_slice(b"retained");
            Ok(Some(8))
        }
    ));
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    use core::future::Future;
    assert_eq!(
        future.as_mut().poll(&mut cx),
        core::task::Poll::Ready(Err(Refusal::Carrier(())))
    );
    assert_eq!(calls.get(), 1);
    let mut carrier = Carrier;
    let mut output = [0; 8];
    let mut future = core::pin::pin!(serve_with(&state, &mut carrier, &mut output, |_, _| Ok(
        Some(9)
    )));
    assert_eq!(
        future.as_mut().poll(&mut cx),
        core::task::Poll::Ready(Err(Refusal::Capacity))
    );
}
#[test]
fn carrier_failure_preserves_observed_execution() {
    struct Disconnected;
    impl MessageTransport for Disconnected {
        type Error = ();
        async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
            output[0] = b'?';
            Ok(&output[..1])
        }
        async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
            Err(())
        }
    }
    let state = State::<1>::from_digest([9; 32]);
    let event = KernelEvent {
        sequence: 1,
        node: NodeId(0),
        port: None,
        request: Some(RequestId(1)),
        kind: KernelEventKind::HostCallCompleted,
    };
    state.record(0, event).unwrap();
    let mut carrier = Disconnected;
    let mut future = core::pin::pin!(serve_one(&state, &mut carrier));
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    use core::future::Future;
    assert_eq!(
        future.as_mut().poll(&mut cx),
        core::task::Poll::Ready(Err(Refusal::Carrier(())))
    );
    assert_eq!(state.fragments[0].get().last, Some(event));
    assert_eq!(state.fragments[0].get().completions, 1);
}
