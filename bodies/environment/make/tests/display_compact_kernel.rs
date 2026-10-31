//! CPU proof of exact selection and unchanged output execution; no GPIO claim.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "display_support/fixture.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "display_support/assigned.rs"]
mod assigned;
use conduit_kernel::{self as k, scheduler::SchedulerStatus, ValueStorage};
#[test]
fn compact_owner_refuses_foreign_scope_then_executes_the_checked_output() {
    for brightness in [0, 3, 7, 8, 255] {
        for frames in [1, 2] {
            for cancel_at in [None, Some(0), Some(3), Some(9)] {
                run(brightness, frames, cancel_at, false);
            }
        }
    }
    for frames in [1, 2] {
        run(3, frames, None, true);
    }
}
fn run(brightness: u8, frames: u32, cancel_at: Option<usize>, native_failure: bool) {
    let mut prepared = assigned::prepare_output(frames, false);
    let mut frame =
        conduit_environment_runtime::display_packets::PreparedDisplayFrame::prepare().unwrap();
    let canonical = frame
        .encode([6, 91, 79, 102, 0, 0], brightness, true)
        .unwrap();
    let endpoint = k::RemoteEndpointId(prepared.input.endpoint());
    let cord = k::CordId(prepared.input.cord());
    let mut packet_encoder =
        conduit_environment_runtime::display_packets::PreparedDisplayPackets::prepare().unwrap();
    let expected = packet_encoder
        .encode_canonical([6, 91, 79, 102, 0, 0], brightness.min(7), true)
        .unwrap()
        .to_vec();
    let ((), memory) = allocation_probe::observe(|| {
        prepared
            .kernel
            .admit_remote_input(endpoint, cord, 0, canonical)
            .unwrap();
        let mut submitted = 1;
        let mut closed = false;
        let mut writes = 0;
        let mut drained = false;
        for step in 0..256 {
            if cancel_at == Some(step) {
                prepared.kernel.cancel().unwrap();
                assert_eq!(prepared.kernel.values().used_items(), 0);
                return;
            }
            if submitted < frames {
                match prepared
                    .kernel
                    .admit_remote_input(endpoint, cord, u64::from(submitted), canonical)
                    .unwrap()
                {
                    k::scheduler::RemoteIngressOutcome::Accepted { .. } => submitted += 1,
                    k::scheduler::RemoteIngressOutcome::Full { .. } => (),
                }
            }
            if submitted == frames && !closed {
                prepared.kernel.close_remote_input(endpoint, cord).unwrap();
                closed = true;
            }
            let status = prepared.kernel.step().unwrap();
            if let Some(request) = prepared.kernel.next_host_request() {
                let fail_native = request.node == prepared.write && native_failure;
                let bytes = prepared.kernel.host_value(request.input.value).unwrap();
                let output = if request.node == prepared.write {
                    assert_eq!(bytes, expected.as_slice());
                    writes += 1;
                    None
                } else {
                    let selected = prepared.selectors[usize::from(request.node.0)]
                        .as_mut()
                        .unwrap()
                        .select(bytes)
                        .unwrap();
                    selected.map(|bytes| {
                        k::BoundedValueRef::new(
                            prepared.kernel.store_host_value(bytes).unwrap(),
                            1024,
                        )
                        .unwrap()
                    })
                };
                prepared
                    .kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: if fail_native {
                                k::HostCallDisposition::Failed
                            } else {
                                k::HostCallDisposition::Completed
                            },
                            output,
                            failure: fail_native.then_some(k::Failure {
                                code: k::FailureCode::HostCallFailed,
                                detail: 1,
                            }),
                        },
                    )
                    .unwrap();
                if fail_native {
                    prepared.kernel.cancel().unwrap();
                    assert_eq!(prepared.kernel.values().used_items(), 0);
                    return;
                }
            }
            if step >= 32 {
                let endpoint = k::RemoteEndpointId(prepared.refusal.endpoint());
                let cord = k::CordId(prepared.refusal.cord());
                if let Some(offer) = prepared.kernel.remote_egress_offer(endpoint, cord).unwrap() {
                    prepared
                        .kernel
                        .remote_egress_accept(endpoint, cord, offer.sequence)
                        .unwrap();
                    prepared
                        .kernel
                        .remote_egress_delivered(endpoint, cord, offer.sequence)
                        .unwrap();
                }
            }
            if status == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        assert_eq!(writes, if brightness <= 7 { frames } else { 0 });
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
