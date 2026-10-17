//! Borrowed inspection of original retained Host Call obligations.
use super::{FixedScheduler, StepBack};
use crate::{NodeId, SignSink, ValueStorage};

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
    D: StepBack<PORTS>,
    S: ValueStorage,
    E: SignSink,
{
    /// Includes unsurfaced requests and completed requests awaiting consumption.
    /// This does not consume a request, retire storage, or certify native idle.
    pub fn has_retained_host_call_for_nodes(&self, nodes: &[NodeId]) -> bool {
        nodes.iter().any(|node| {
            let index = usize::from(node.0);
            if index >= self.active_nodes {
                return false;
            }
            let pending = self.indexed_host_call(index);
            #[cfg(test)]
            if pending.is_some() {
                let mut counts = self.scan_counts.get();
                counts.retained_pending_slots += 1;
                self.scan_counts.set(counts);
            }
            pending.is_some_and(|pending| pending.request.node == *node)
        })
    }
}
