//! Ownership handoff after scheduler execution is terminal and drained.
use super::{FixedScheduler, OperationDriver, SchedulerError, StepOperation};
use crate::{KernelEventKind, NodeId, SignSink, ValueStorage};

/// Retains the original drivers, storage and evidence without cloning State.
/// This is an ownership boundary, not semantic migration permission. `cancelled`
/// records scheduler cancellation; terminal meaning remains in the retained Signs.
pub struct RetiredExecution<D, S, E, const NODES: usize> {
    pub drivers: [D; NODES],
    pub values: S,
    pub signs: E,
    pub active_nodes: usize,
    pub decisions: u32,
    pub cancelled: bool,
}

impl<O, const PORTS: usize> OperationDriver<O, PORTS> {
    /// Consumes an owned driver. Active schedulers expose only borrowed drivers.
    pub fn into_operation(self) -> O {
        self.operation
    }
}

impl<
        D,
        S,
        E,
        const NODES: usize,
        const CORDS: usize,
        const PORTS: usize,
        const QUEUE_SLOTS: usize,
        const ROUTE_SLOTS: usize,
        const ROUTE_TARGETS: usize,
        const HOST_BINDING_SLOTS: usize,
        const PENDING_REQUESTS: usize,
    >
    FixedScheduler<
        D,
        S,
        E,
        NODES,
        CORDS,
        PORTS,
        QUEUE_SLOTS,
        ROUTE_SLOTS,
        ROUTE_TARGETS,
        HOST_BINDING_SLOTS,
        PENDING_REQUESTS,
    >
where
    D: StepOperation<PORTS>,
    S: ValueStorage,
    E: SignSink,
{
    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Ok(());
        }
        self.ensure_sign_capacity(2)?;
        self.signs.record(
            NodeId(0),
            None,
            None,
            KernelEventKind::CancellationRequested,
        )?;
        for (node, driver) in self.drivers[..self.active_nodes].iter_mut().enumerate() {
            if !self.completed[node] {
                driver.cancel();
            }
        }
        self.values.clear();
        self.pending_host_operations.fill(None);
        self.queue_slots.fill(None);
        for cord in &mut self.cords[..self.active_cords] {
            cord.head = 0;
            cord.len = 0;
            cord.queued_bytes = 0;
            cord.producer_closed = true;
            cord.offered_remote_sequence = None;
            cord.remote_accepted = false;
        }
        self.ready.fill(false);
        self.cancelled = true;
        self.signs
            .record(NodeId(0), None, None, KernelEventKind::RunCancelled)?;
        Ok(())
    }

    /// Refuses active/quiescent execution and outstanding queue/host ownership.
    /// Refusal returns the exact original scheduler so execution may continue.
    #[allow(
        clippy::result_large_err,
        reason = "Refusal must return fixed-storage ownership without allocating or dropping the active scheduler"
    )]
    pub fn try_retire(self) -> Result<RetiredExecution<D, S, E, NODES>, Self> {
        if (!self.cancelled && !self.completed[..self.active_nodes].iter().all(|done| *done))
            || self.pending_host_operations.iter().any(Option::is_some)
            || self.queue_slots.iter().any(Option::is_some)
            || self.cords[..self.active_cords]
                .iter()
                .any(|cord| cord.len != 0 || cord.offered_remote_sequence.is_some())
        {
            return Err(self);
        }
        Ok(RetiredExecution {
            drivers: self.drivers,
            values: self.values,
            signs: self.signs,
            active_nodes: self.active_nodes,
            decisions: self.decisions,
            cancelled: self.cancelled,
        })
    }
}
