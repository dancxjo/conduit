//! Generic kernel evidence, with fixture retained evidence digests. This does
//! not manufacture canonical build receipts or prove an installed native UI.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_body_make::{deployment_runtime::*, deployment_runtime_catalog::*};
use conduit_kernel::{scheduler::*, *};
type TestKernel<D> =
    FixedScheduler<D, FixedValueStore<8, 128>, FixedSignLog<128>, 1, 2, 1, 2, 1, 1, 1, 8>;
type Kernel = TestKernel<CrecheBack>;
fn prepared_kernel<D: StepBack<1>>(back: D) -> TestKernel<D> {
    let mut routes = FixedRoutes::<1, 1>::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(1),
                sink: CordEndpoint::Remote(RemoteEndpointId(1)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::<1>::new(1);
    calls
        .install(NodeId(0), creche_kernel_call_binding(HostCallId(0)))
        .unwrap();
    calls.seal().unwrap();
    let capacity = |slot_start| CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity: 128,
        pressure_policy: Default::default(),
    };
    let signs = FixedSignLog::<128>::new_with_remote_storage(
        128 * core::mem::size_of::<KernelEvent>() as u32,
        32,
        remote_sign_storage_bytes(32).unwrap(),
    )
    .unwrap();
    TestKernel::new_with_host_calls(
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_fuel: 16,
        }],
        [
            CordSpec::remote_ingress(
                CordId(0),
                RemoteEndpointId(0),
                (NodeId(0), PortId(0)),
                capacity(0),
            ),
            CordSpec::remote_egress(
                CordId(1),
                (NodeId(0), PortId(0)),
                RemoteEndpointId(1),
                capacity(1),
            ),
        ],
        routes,
        calls,
        [back],
        FixedValueStore::new(1024).unwrap(),
        signs,
    )
    .unwrap()
}
fn kernel(request: CrecheRequest) -> Kernel {
    let mut kernel = prepared_kernel(CrecheBack::new(HostCallId(0), request));
    kernel
        .admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &request.0)
        .unwrap();
    kernel
}
#[test]
fn exact_request_crosses_one_host_call_and_output_retains_exact_evidence() {
    let request = CrecheRequest([5; 32]);
    let mut kernel = kernel(request);
    let (_, memory) = allocation_probe::observe(|| {
        kernel.step().unwrap();
        let call = kernel.next_host_request().unwrap();
        assert_eq!(kernel.values().get(call.input.value).unwrap(), request.0);
        assert!(kernel.next_host_request().is_none());
        let outcome = CrecheOutcome {
            request,
            disposition: CrecheDisposition::Prepared,
            evidence_digest: [9; 32],
        };
        let value = kernel.store_host_value(&outcome.encode()).unwrap();
        kernel
            .complete_host_call(
                call.node,
                call.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(BoundedValueRef::new(value, 65).unwrap()),
                    failure: None,
                },
            )
            .unwrap();
        kernel.step().unwrap();
        let offered = kernel
            .remote_egress_offer(RemoteEndpointId(1), CordId(1))
            .unwrap()
            .unwrap();
        assert_eq!(
            CrecheOutcome::decode(kernel.values().get(offered.value).unwrap()),
            Some(outcome)
        );
        // Unaccepted remote output remains retained; no retry or second make effect.
        for _ in 0..4 {
            kernel.step().unwrap();
            assert_eq!(
                kernel
                    .remote_egress_offer(RemoteEndpointId(1), CordId(1))
                    .unwrap()
                    .unwrap()
                    .value,
                offered.value
            );
            assert!(kernel.next_host_request().is_none());
        }
    });
    assert_eq!(
        (
            memory.allocations,
            memory.reallocations,
            memory.deallocations
        ),
        (0, 0, 0)
    );
}
#[test]
fn denied_and_cancelled_host_work_cannot_become_prepared_output() {
    for (disposition, expected) in [
        (HostCallDisposition::Denied, FailureCode::HostCallDenied),
        (HostCallDisposition::Cancelled, FailureCode::Cancelled),
    ] {
        let mut kernel = kernel(CrecheRequest([5; 32]));
        kernel.step().unwrap();
        let call = kernel.next_host_request().unwrap();
        kernel
            .complete_host_call(
                call.node,
                call.request,
                HostCallOutcome {
                    disposition,
                    output: None,
                    failure: None,
                },
            )
            .unwrap();
        let status = kernel.step();
        assert_eq!(
            status,
            Err(SchedulerError::BackFailed(Failure {
                code: expected,
                detail: 0
            }))
        );
        assert!(kernel
            .remote_egress_offer(RemoteEndpointId(1), CordId(1))
            .unwrap()
            .is_none());
    }
}
#[test]
fn hard_kernel_cancellation_rejects_late_completion_and_releases_request_ownership() {
    let mut kernel = kernel(CrecheRequest([5; 32]));
    let (_, memory) = allocation_probe::observe(|| {
        kernel.step().unwrap();
        let call = kernel.next_host_request().unwrap();
        // Hard cancellation revokes kernel ownership. The installed execution owner
        // must cancel/drain platform effects before taking this terminal step;
        // cancel() does not dispatch an adapter cancellation on its own.
        kernel.cancel().unwrap();
        assert_eq!(kernel.pending_host_call_count(), 0);
        assert_eq!(kernel.values().used_items(), 0);
        assert_eq!(kernel.step(), Ok(SchedulerStatus::Cancelled));
        assert_eq!(
            kernel.remote_egress_offer(RemoteEndpointId(1), CordId(1)),
            Err(SchedulerError::Cancelled)
        );
        assert_eq!(
            kernel.complete_host_call(
                call.node,
                call.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Cancelled,
                    output: None,
                    failure: None,
                }
            ),
            Err(SchedulerError::HostCallCompletionRejected)
        );
        assert!(kernel.next_host_request().is_none());
    });
    assert_eq!(
        (
            memory.allocations,
            memory.reallocations,
            memory.deallocations
        ),
        (0, 0, 0)
    );
}

#[test]
fn another_encounters_receipt_cannot_complete_this_request() {
    let mut kernel = kernel(CrecheRequest([5; 32]));
    kernel.step().unwrap();
    let call = kernel.next_host_request().unwrap();
    let wrong = CrecheOutcome {
        request: CrecheRequest([6; 32]),
        disposition: CrecheDisposition::Prepared,
        evidence_digest: [9; 32],
    };
    let output = kernel.store_host_value(&wrong.encode()).unwrap();
    kernel
        .complete_host_call(
            call.node,
            call.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 65).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    assert_eq!(
        kernel.step(),
        Err(SchedulerError::BackFailed(Failure {
            code: FailureCode::InvalidInput,
            detail: 6
        }))
    );
    assert!(kernel
        .remote_egress_offer(RemoteEndpointId(1), CordId(1))
        .unwrap()
        .is_none());
}

#[test]
fn empty_closed_request_is_invalid_instead_of_waiting_forever() {
    let mut kernel = prepared_kernel(CrecheBack::new(HostCallId(0), CrecheRequest([5; 32])));
    kernel
        .close_remote_input(RemoteEndpointId(0), CordId(0))
        .unwrap();
    assert_eq!(
        kernel.step(),
        Err(SchedulerError::BackFailed(Failure {
            code: FailureCode::InvalidInput,
            detail: 8
        }))
    );
    assert!(kernel.next_host_request().is_none());
}

fn complete_fixture<D: StepBack<1>>(kernel: &mut TestKernel<D>, request: CrecheRequest) {
    let call = kernel.next_host_request().unwrap();
    let result = CrecheOutcome {
        request,
        disposition: CrecheDisposition::Prepared,
        evidence_digest: [9; 32],
    };
    let output = kernel.store_host_value(&result.encode()).unwrap();
    kernel
        .complete_host_call(
            call.node,
            call.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 65).unwrap()),
                failure: None,
            },
        )
        .unwrap();
}
fn drain_output<D: StepBack<1>>(kernel: &mut TestKernel<D>) {
    let offer = kernel
        .remote_egress_offer(RemoteEndpointId(1), CordId(1))
        .unwrap()
        .unwrap();
    kernel
        .remote_egress_accept(RemoteEndpointId(1), CordId(1), offer.sequence)
        .unwrap();
    kernel
        .remote_egress_delivered(RemoteEndpointId(1), CordId(1), offer.sequence)
        .unwrap();
}
#[test]
fn exactly_one_outcome_completes_without_waiting_for_request_close() {
    let request = CrecheRequest([5; 32]);
    let mut kernel = kernel(request);
    kernel.step().unwrap();
    complete_fixture(&mut kernel, request);
    kernel.step().unwrap();
    drain_output(&mut kernel);
    kernel.step().unwrap();
    assert!(kernel
        .signs()
        .events()
        .any(|event| event.kind == KernelEventKind::BackCompleted));
    assert!(kernel.next_host_request().is_none());
}

/// Artificial preceding producer value fills the *same* bounded output cord,
/// so the actual CrecheBack must await capacity before consuming its completion.
/// This wrapper is a pressure fixture, never an installed Crèche implementation.
struct OccupiedOutput {
    inner: CrecheBack,
    prefixed: bool,
    staged: bool,
}
impl StepBack<1> for OccupiedOutput {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        if !self.prefixed {
            io.send_canonical(PortId(0), CanonicalValue::new(&[99]).unwrap())
                .unwrap();
            self.staged = true;
            return StepOutcome::Progress;
        }
        self.inner.step(io, bytes)
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.prefixed = true;
            self.staged = false;
        } else {
            <CrecheBack as StepBack<1>>::step_committed(&mut self.inner);
        }
    }
}
#[test]
fn blocked_output_retains_host_completion_until_capacity_returns() {
    let request = CrecheRequest([5; 32]);
    let mut kernel = prepared_kernel(OccupiedOutput {
        inner: CrecheBack::new(HostCallId(0), request),
        prefixed: false,
        staged: false,
    });
    kernel
        .admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &request.0)
        .unwrap();
    kernel.step().unwrap(); // fixture occupies output
    kernel.step().unwrap(); // real Back requests admitted work
    complete_fixture(&mut kernel, request);
    for _ in 0..4 {
        kernel.step().unwrap();
        assert_eq!(
            kernel.pending_host_call_count(),
            1,
            "completion must remain unconsumed under pressure"
        );
        assert!(kernel.next_host_request().is_none(), "no repeated effect");
        let offered = kernel
            .remote_egress_offer(RemoteEndpointId(1), CordId(1))
            .unwrap()
            .unwrap();
        assert_eq!(kernel.values().get(offered.value).unwrap(), &[99]);
    }
    drain_output(&mut kernel);
    kernel.step().unwrap();
    assert_eq!(kernel.pending_host_call_count(), 0);
    let offered = kernel
        .remote_egress_offer(RemoteEndpointId(1), CordId(1))
        .unwrap()
        .unwrap();
    let outcome = CrecheOutcome::decode(kernel.values().get(offered.value).unwrap()).unwrap();
    assert_eq!(outcome.request, request);
    assert_eq!(outcome.disposition, CrecheDisposition::Prepared);
}

#[test]
fn installed_back_pins_first_request_and_rejects_another_requests_completion() {
    for matching in [true, false] {
        let request = CrecheRequest([17; 32]);
        let mut kernel = prepared_kernel(CrecheBack::for_admitted_host(HostCallId(0)));
        kernel
            .admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &request.0)
            .unwrap();
        kernel.step().unwrap();
        complete_fixture(
            &mut kernel,
            if matching {
                request
            } else {
                CrecheRequest([18; 32])
            },
        );
        if matching {
            kernel.step().unwrap();
            let offer = kernel
                .remote_egress_offer(RemoteEndpointId(1), CordId(1))
                .unwrap()
                .unwrap();
            assert_eq!(
                CrecheOutcome::decode(kernel.values().get(offer.value).unwrap())
                    .unwrap()
                    .request,
                request
            );
        } else {
            assert_eq!(
                kernel.step(),
                Err(SchedulerError::BackFailed(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 6
                }))
            );
            assert!(kernel
                .remote_egress_offer(RemoteEndpointId(1), CordId(1))
                .unwrap()
                .is_none());
        }
    }
}
