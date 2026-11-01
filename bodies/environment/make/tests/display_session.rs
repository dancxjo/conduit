//! Complete checked output graph reuse. Native completion is a CPU fixture.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "display_support/assigned.rs"]
mod assigned;
#[path = "display_support/fixture.rs"]
mod fixture;
use conduit_environment_runtime::{display_output::DisplaySignBudget, display_packets::*};
use conduit_kernel::{
    self as k, scheduler::prepared_session::*, scheduler::SchedulerStatus, SignQuery, ValueStorage,
};
type Log = k::FixedSignLog<64>;
fn log(_: DisplaySignBudget) -> Log {
    fresh_log()
}
fn fresh_log() -> Log {
    Log::new_with_remote_storage(
        64 * core::mem::size_of::<k::KernelEvent>() as u32,
        6,
        k::remote_sign_storage_bytes(6).unwrap(),
    )
    .unwrap()
}
#[test]
fn checked_output_reuses_960_frames_with_conditional_refusals_and_no_allocations() {
    for mixed in [false, true] {
        let prepared = assigned::prepare_with_signs(960, true, log);
        let input = OwnedValueBoundary {
            endpoint: k::RemoteEndpointId(prepared.input.endpoint()),
            cord: k::CordId(prepared.input.cord()),
        };
        let refusal = OwnedValueBoundary {
            endpoint: k::RemoteEndpointId(prepared.refusal.endpoint()),
            cord: k::CordId(prepared.refusal.cord()),
        };
        let write = prepared.write;
        let mut selectors = prepared.selectors;
        let mut session = PreparedKernelSession::new_with_owned_optional_value_outputs(
            prepared.kernel,
            &[input, refusal],
            &[refusal],
            6,
        )
        .unwrap_or_else(|_| panic!("exact checked fore"));
        let address = session.kernel() as *const _;
        let mut frame = PreparedDisplayFrame::prepare().unwrap();
        let valid = frame
            .encode([6, 91, 79, 102, 0, 0], 3, true)
            .unwrap()
            .to_vec();
        let invalid = frame
            .encode([6, 91, 79, 102, 0, 0], 8, true)
            .unwrap()
            .to_vec();
        let mut packets = PreparedDisplayPackets::prepare().unwrap();
        let expected = packets
            .encode_canonical([6, 91, 79, 102, 0, 0], 3, true)
            .unwrap()
            .to_vec();
        let mut writes = 0;
        let mut refusals = 0;
        let mut requests = [0; 4];
        let mut largest_log = 0;
        let ((), memory) = allocation_probe::observe(|| {
            for sequence in 0..960 {
                let rejected = mixed && sequence % 2 == 1;
                session
                    .admit_owned_input(
                        input.endpoint,
                        input.cord,
                        sequence,
                        if rejected { &invalid } else { &valid },
                    )
                    .unwrap();
                session
                    .close_owned_input(input.endpoint, input.cord)
                    .unwrap();
                let mut drained = false;
                for _ in 0..128 {
                    if session.step().unwrap() == SchedulerStatus::Drained {
                        drained = true;
                        break;
                    }
                    if let Some(request) = session.next_host_request() {
                        let node = usize::from(request.node.0);
                        assert_eq!(request.request, k::RequestId(requests[node]));
                        requests[node] += 1;
                        let bytes = session.kernel().host_value(request.input.value).unwrap();
                        let output = if request.node == write {
                            assert!(!rejected);
                            assert_eq!(bytes, expected);
                            writes += 1;
                            None
                        } else {
                            selectors[node]
                                .as_mut()
                                .unwrap()
                                .select(bytes)
                                .unwrap()
                                .map(|bytes| {
                                    k::BoundedValueRef::new(
                                        session.store_host_value(bytes).unwrap(),
                                        1024,
                                    )
                                    .unwrap()
                                })
                        };
                        session
                            .complete_host_call(
                                request.node,
                                request.request,
                                k::HostCallOutcome {
                                    disposition: k::HostCallDisposition::Completed,
                                    output,
                                    failure: None,
                                },
                            )
                            .unwrap();
                    }
                    if let Some(offer) = session
                        .owned_output(refusal.endpoint, refusal.cord)
                        .unwrap()
                    {
                        assert!(rejected);
                        assert_eq!(offer.sequence, refusals);
                        session
                            .accept_owned_output(refusal.endpoint, refusal.cord, refusals)
                            .unwrap();
                        refusals += 1;
                    }
                }
                assert!(drained);
                assert_eq!(session.kernel() as *const _, address);
                assert_eq!(session.kernel().values().used_items(), 0);
                assert!(session.kernel().signs().retention_gap().is_none());
                largest_log = largest_log.max(session.kernel().signs().events().count());
                if sequence < 959 {
                    let old = session
                        .begin_next([(); 4], fresh_log())
                        .unwrap_or_else(|e| panic!("normal checked output drain: {:?}", e.reason));
                    assert!(old.signs.retention_gap().is_none());
                    assert!(old.signs.events().count() > 0);
                    let mut wire =
                        [0; conduit_environment_runtime::display_evidence::MAXIMUM_BYTES];
                    let size = conduit_environment_runtime::display_evidence::encode(
                        sequence,
                        old.signs.events(),
                        &old.signs,
                        &mut wire,
                    )
                    .unwrap();
                    let page =
                        conduit_environment_runtime::display_evidence::Page::parse(&wire[..size])
                            .unwrap();
                    assert_eq!(page.invocation(), sequence);
                    assert!(page.records().map(|r| r.event).eq(old.signs.events()));
                    for record in page.records() {
                        assert_eq!(
                            record.remote,
                            old.signs.remote_identity(record.event.sequence)
                        );
                    }
                }
            }
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(writes, if mixed { 480 } else { 960 });
        assert_eq!(refusals, if mixed { 480 } else { 0 });
        println!("mixed={mixed}: maximum complete invocation log={largest_log}");
    }
}

#[test]
fn output_session_failure_and_cancellation_preserve_nonrearmable_state() {
    for cancel in [true, false] {
        let prepared = assigned::prepare_with_signs(2, true, log);
        let input = OwnedValueBoundary {
            endpoint: k::RemoteEndpointId(prepared.input.endpoint()),
            cord: k::CordId(prepared.input.cord()),
        };
        let refusal = OwnedValueBoundary {
            endpoint: k::RemoteEndpointId(prepared.refusal.endpoint()),
            cord: k::CordId(prepared.refusal.cord()),
        };
        let mut session = PreparedKernelSession::new_with_owned_optional_value_outputs(
            prepared.kernel,
            &[input, refusal],
            &[refusal],
            6,
        )
        .unwrap_or_else(|_| panic!("exact checked fore"));
        let mut frame = PreparedDisplayFrame::prepare().unwrap();
        session
            .admit_owned_input(
                input.endpoint,
                input.cord,
                0,
                frame.encode([6, 91, 79, 102, 0, 0], 3, true).unwrap(),
            )
            .unwrap();
        session
            .close_owned_input(input.endpoint, input.cord)
            .unwrap();
        let request = (0..16)
            .find_map(|_| {
                session.step().unwrap();
                session.next_host_request()
            })
            .expect("selector request");
        if cancel {
            session.cancel().unwrap();
        } else {
            session
                .complete_host_call(
                    request.node,
                    request.request,
                    k::HostCallOutcome {
                        disposition: k::HostCallDisposition::Failed,
                        output: None,
                        failure: Some(k::Failure {
                            code: k::FailureCode::HostCallFailed,
                            detail: 17,
                        }),
                    },
                )
                .unwrap();
            for _ in 0..16 {
                if session.step().is_err()
                    || session
                        .kernel()
                        .signs()
                        .contains_kind(k::KernelEventKind::BackFailed)
                {
                    break;
                }
            }
            assert!(session
                .kernel()
                .signs()
                .contains_kind(k::KernelEventKind::BackFailed));
        }
        assert_eq!(
            session
                .begin_next([(); 4], fresh_log())
                .err()
                .unwrap()
                .reason,
            SessionRefusal::NotNormallyDrained
        );
    }
}
