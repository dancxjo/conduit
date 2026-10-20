//! A forwarding adapter: both fixture profiles advance the same execution kernel.
use super::{prepared::Kernel, *};
use conduit_kernel::{
    scheduler::{
        prepared_session::PreparedKernelSession, HostCallRequest, SchedulerError, SchedulerStatus,
    },
    HostCallOutcome, NodeId, RequestId, ValueRef,
};
pub(super) type Session =
    PreparedKernelSession<Driver, ObservedValues, HostedSignLog, N, C, PORTS, C, C, C, N, 256>;
pub(super) enum ExecutionKernel {
    Fresh(Box<Kernel>),
    Reusable(Box<Session>),
}
impl std::ops::Deref for ExecutionKernel {
    type Target = Kernel;
    fn deref(&self) -> &Kernel {
        match self {
            Self::Fresh(k) => k,
            Self::Reusable(k) => k.kernel(),
        }
    }
}
impl ExecutionKernel {
    pub(super) fn cancel(&mut self) -> Result<(), SchedulerError> {
        match self {
            Self::Fresh(kernel) => kernel.cancel(),
            Self::Reusable(kernel) => kernel.cancel(),
        }
    }
    pub(super) fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        match self {
            Self::Fresh(k) => k.step(),
            Self::Reusable(k) => k.step(),
        }
    }
    pub(super) fn next_host_request(&mut self) -> Option<HostCallRequest> {
        match self {
            Self::Fresh(k) => k.next_host_request(),
            Self::Reusable(k) => k.next_host_request(),
        }
    }
    pub(super) fn store_host_value(&mut self, bytes: &[u8]) -> Result<ValueRef, SchedulerError> {
        match self {
            Self::Fresh(k) => k.store_host_value(bytes),
            Self::Reusable(k) => k.store_host_value(bytes),
        }
    }
    pub(super) fn complete_host_call(
        &mut self,
        node: NodeId,
        request: RequestId,
        outcome: HostCallOutcome,
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Fresh(k) => k.complete_host_call(node, request, outcome),
            Self::Reusable(k) => k.complete_host_call(node, request, outcome),
        }
    }
}
