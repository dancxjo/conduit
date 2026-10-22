//! Outlined calls into the existing kernel; no new scheduling or effect policy.
use super::BenchKernel;
use conduit_kernel::{scheduler::SchedulerStatus, HostCallOutcome, NodeId, RequestId};
#[inline(never)]
pub(super) fn advance(kernel: &mut BenchKernel<'_>) -> SchedulerStatus {
    kernel.step().unwrap()
}
#[inline(never)]
pub(super) fn complete(
    kernel: &mut BenchKernel<'_>,
    node: NodeId,
    request: RequestId,
    outcome: HostCallOutcome,
) {
    kernel.complete_host_call(node, request, outcome).unwrap();
}
