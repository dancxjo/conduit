//! Actual production-kernel execution with fixture completion evidence. This
//! does not establish builder custody, image production, a Body Play or join.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "common/preparation_kernel.rs"]
mod preparation_kernel;
use conduit_body_make::{stem_runtime::*, stem_runtime_catalog::*};
use conduit_kernel::{scheduler::*, *};
use preparation_kernel::{prepared_kernel, TestKernel};

type Kernel = TestKernel<StemBack>;
fn kernel() -> Kernel {
    let mut kernel = prepared_kernel(
        StemBack::for_admitted_host(HostCallId(0)),
        stem_kernel_call_binding(HostCallId(0)),
    );
    kernel
        .admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &[5; 32])
        .unwrap();
    kernel
}
fn outcome(disposition: StemDisposition) -> StemOutcome {
    StemOutcome {
        request: StemRequest([5; 32]),
        disposition,
        evidence_digest: [9; 32],
    }
}
fn complete(kernel: &mut Kernel, call: HostCallRequest, outcome: StemOutcome) {
    let value = kernel.store_host_value(&outcome.encode()).unwrap();
    kernel
        .complete_host_call(
            call.node,
            call.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(value, STEM_OUTCOME_BYTES as u32).unwrap()),
                failure: None,
            },
        )
        .unwrap();
}

#[test]
fn every_classified_outcome_crosses_one_call_and_remains_retained_until_accepted() {
    for disposition in [
        StemDisposition::Prepared,
        StemDisposition::UnavailableBuilder,
        StemDisposition::UnsupportedTarget,
        StemDisposition::StaleInvitation,
        StemDisposition::Denied,
        StemDisposition::Cancelled,
        StemDisposition::Failed,
        StemDisposition::InvalidSelection,
    ] {
        let mut kernel = kernel();
        let (_, memory) = allocation_probe::observe(|| {
            kernel.step().unwrap();
            let call = kernel.next_host_request().unwrap();
            assert_eq!(kernel.values().get(call.input.value).unwrap(), [5; 32]);
            assert!(kernel.next_host_request().is_none());
            complete(&mut kernel, call, outcome(disposition));
            kernel.step().unwrap();
            let offered = kernel
                .remote_egress_offer(RemoteEndpointId(1), CordId(1))
                .unwrap()
                .unwrap();
            assert_eq!(
                StemOutcome::decode(kernel.values().get(offered.value).unwrap()),
                Some(outcome(disposition))
            );
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
}

#[test]
fn another_request_cannot_complete_the_admitted_propagation() {
    let mut kernel = kernel();
    kernel.step().unwrap();
    let call = kernel.next_host_request().unwrap();
    let mut other = outcome(StemDisposition::Prepared);
    other.request = StemRequest([7; 32]);
    complete(&mut kernel, call, other);
    assert!(matches!(
        kernel.step(),
        Err(SchedulerError::BackFailed(Failure {
            code: FailureCode::InvalidInput,
            ..
        }))
    ));
}

#[test]
fn mechanism_denial_failure_and_cancellation_do_not_become_prepared_output() {
    for (disposition, expected) in [
        (HostCallDisposition::Denied, FailureCode::HostCallDenied),
        (HostCallDisposition::Failed, FailureCode::HostCallFailed),
        (HostCallDisposition::Cancelled, FailureCode::Cancelled),
    ] {
        let mut kernel = kernel();
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
        assert!(matches!(
            kernel.step(),
            Err(SchedulerError::BackFailed(failure)) if failure.code == expected
        ));
        assert!(kernel
            .remote_egress_offer(RemoteEndpointId(1), CordId(1))
            .unwrap()
            .is_none());
    }
}
