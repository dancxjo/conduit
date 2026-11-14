//! A validated schema7 local Fore shares queue mechanics with external endpoints,
//! but has no Line delivery or remote-lifecycle custody to manufacture.
use super::*;

impl<
        D,
        S,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const A: usize,
        const TERMINAL_ROWS: usize,
    > FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A, TERMINAL_ROWS>
where
    D: StepBack<P>,
    S: ValueStorage,
    E: SignSink,
{
    pub(super) fn ensure_boundary_remote_capacity(
        &self,
        cord: usize,
        count: usize,
    ) -> Result<(), SchedulerError> {
        if self.cord_specs[cord].is_local_fore() {
            Ok(())
        } else {
            self.ensure_remote_sign_capacity(count)
        }
    }
    pub(super) fn record_boundary_sign(
        &mut self,
        cord: usize,
        node: NodeId,
        port: PortId,
        kind: KernelEventKind,
        identity: crate::RemoteLifecycleIdentity,
    ) -> Result<crate::KernelEvent, SchedulerError> {
        if !self.cord_specs[cord].is_local_fore() {
            return self
                .signs
                .record_remote(node, port, kind, identity)
                .map_err(Into::into);
        }
        let local = match kind {
            KernelEventKind::RemoteInputAdmitted => KernelEventKind::ValueStored,
            KernelEventKind::RemoteValueOffered | KernelEventKind::RemoteValueAccepted => {
                KernelEventKind::ValueRouted
            }
            KernelEventKind::RemoteValueDelivered => KernelEventKind::ValueConsumed,
            KernelEventKind::RemoteInputClosed | KernelEventKind::RemoteOutputClosed => {
                if self.cords[cord].producer_abnormal {
                    KernelEventKind::InputAbnormal
                } else {
                    KernelEventKind::InputClosed
                }
            }
            _ => return Err(SchedulerError::InvalidPlan),
        };
        self.signs
            .record(node, Some(port), None, local)
            .map_err(Into::into)
    }
}
