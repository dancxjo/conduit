//! CPU proof: repeated owned Values retain the original electrical work ceiling.
//! Host completions are fixtures; this does not claim physical output.
#![cfg(feature = "prepared-write-session")]
use conduit_kernel::{self as k, scheduler::prepared_session::*, scheduler::*, ValueStorage};
use conduit_tm1637_wire::WriteBack;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
type Log = k::FixedSignLog<64>;
type Session =
    PreparedKernelSession<WriteBack, k::FixedValueStore<2, 32>, Log, 1, 1, 1, 1, 1, 1, 1, 1>;
fn log() -> Log {
    Log::new_with_remote_storage(
        64 * core::mem::size_of::<k::KernelEvent>() as u32,
        8,
        k::remote_sign_storage_bytes(8).unwrap(),
    )
    .unwrap()
}
fn session() -> Session {
    session_with_limit(2)
}
fn session_with_limit(limit: u32) -> Session {
    let mut routes = k::FixedRoutes::new(1);
    routes.seal().unwrap();
    let mut bindings = k::FixedHostCallBindings::new(1);
    bindings
        .install(
            k::NodeId(0),
            k::HostCallBinding {
                call: k::HostCallId(0),
                maximum_input_bytes: conduit_tm1637_wire::MAXIMUM_BYTES,
                maximum_output_bytes: 0,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let kernel = FixedScheduler::new_with_host_calls(
        [NodeSpec {
            input_cords: [Some(k::CordId(0))],
            maximum_step_fuel: 8,
        }],
        [CordSpec::remote_ingress(
            k::CordId(0),
            k::RemoteEndpointId(0),
            (k::NodeId(0), k::PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 32,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        bindings,
        [WriteBack::new(limit)],
        k::FixedValueStore::new(64).unwrap(),
        log(),
    )
    .unwrap();
    Session::new_with_owned_value_boundaries(
        Box::new(kernel),
        &[OwnedValueBoundary {
            cord: k::CordId(0),
            endpoint: k::RemoteEndpointId(0),
        }],
        8,
    )
    .unwrap_or_else(|_| panic!("fresh exact kernel"))
}
#[test]
fn repeated_960_invocations_have_fixed_storage_and_no_execution_allocation() {
    use k::{SignQuery, SignSink};
    let mut s = session_with_limit(960);
    let address = s.kernel() as *const _;
    let ((), memory) = allocation_probe::observe(|| {
        for sequence in 0..960 {
            invoke(&mut s, sequence, false);
            assert_eq!(s.kernel() as *const _, address);
            assert_eq!(s.kernel().values().used_items(), 0);
            assert!(s.kernel().signs().retention_gap().is_none());
            if sequence < 959 {
                // The CPU consumer explicitly takes custody of the old log.
                // Firmware still needs an admitted evidence transfer/ack path.
                let previous = s
                    .begin_next([()], log())
                    .unwrap_or_else(|_| panic!("normal drain within admitted ceiling"));
                assert!(!previous.signs.is_empty());
                assert!(previous.signs.retention_gap().is_none());
            }
        }
        assert!(s.begin_next([()], log()).is_err());
    });
    assert_eq!(memory.allocations, 0);
    assert_eq!(memory.reallocations, 0);
}
fn invoke(s: &mut Session, sequence: u64, fail: bool) {
    assert!(matches!(
        s.admit_owned_input(k::RemoteEndpointId(0), k::CordId(0), sequence, &[1, 2])
            .unwrap(),
        RemoteIngressOutcome::Accepted { .. }
    ));
    s.close_owned_input(k::RemoteEndpointId(0), k::CordId(0))
        .unwrap();
    let mut requested = false;
    for _ in 0..16 {
        let outcome = s.step();
        if fail && outcome.is_err() {
            assert!(requested);
            return;
        }
        if outcome.unwrap() == SchedulerStatus::Drained {
            assert!(requested);
            return;
        }
        if let Some(request) = s.next_host_request() {
            assert!(!requested);
            assert_eq!(request.request, k::RequestId(sequence as u32));
            assert_eq!(
                s.kernel().values().get(request.input.value).unwrap(),
                &[1, 2]
            );
            requested = true;
            s.complete_host_call(
                request.node,
                request.request,
                k::HostCallOutcome {
                    disposition: if fail {
                        k::HostCallDisposition::Failed
                    } else {
                        k::HostCallDisposition::Completed
                    },
                    output: None,
                    failure: fail.then_some(k::Failure {
                        code: k::FailureCode::HostCallDenied,
                        detail: 17,
                    }),
                },
            )
            .unwrap();
        }
    }
    panic!("invocation did not terminate")
}
#[test]
fn repeated_values_preserve_requests_evidence_and_total_write_ceiling() {
    let mut s = session();
    let address = s.kernel() as *const _;
    invoke(&mut s, 0, false);
    let evidence = s.kernel().signs().events().collect::<Vec<_>>();
    let previous = s
        .begin_next([()], log())
        .unwrap_or_else(|_| panic!("normal drain"));
    assert_eq!(previous.signs.events().collect::<Vec<_>>(), evidence);
    assert_eq!(s.kernel() as *const _, address);
    assert_eq!(s.kernel().values().used_items(), 0);
    invoke(&mut s, 1, false);
    let before = s.kernel().signs().events().collect::<Vec<_>>();
    let refusal = s.begin_next([()], log()).err().unwrap();
    assert_eq!(
        refusal.reason,
        SessionRefusal::BackNotReady { node: k::NodeId(0) }
    );
    assert_eq!(s.kernel().signs().events().collect::<Vec<_>>(), before);
}
#[test]
fn cancellation_and_failed_electrical_completion_cannot_rearm() {
    let mut cancelled = session();
    cancelled.cancel().unwrap();
    assert_eq!(
        cancelled.begin_next([()], log()).err().unwrap().reason,
        SessionRefusal::NotNormallyDrained
    );
    let mut failed = session();
    invoke(&mut failed, 0, true);
    assert_eq!(
        failed.begin_next([()], log()).err().unwrap().reason,
        SessionRefusal::NotNormallyDrained
    );
}
