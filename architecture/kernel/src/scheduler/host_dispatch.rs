//! Ordered adapter selection over the admitted pending Host Call table.
use super::*;

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
    pub fn next_host_request(&mut self) -> Option<HostCallRequest> {
        self.next_host_request_matching(|_| true)
    }

    /// Selects the next undispatched request accepted by the host adapter.
    /// Non-matching requests remain undispatched and retain their exact order
    /// for a later call.
    pub fn next_host_request_matching(
        &mut self,
        mut accepts: impl FnMut(&HostCallRequest) -> bool,
    ) -> Option<HostCallRequest> {
        if self.undispatched_host_count == 0 {
            return None;
        }
        let mut remaining = self.undispatched_host_count;
        // There is at most one retained call per active node. First-free
        // allocation therefore cannot populate a slot beyond this prefix,
        // even after arbitrary completion consumption and slot reuse.
        // Every slot before the allocation hint is occupied. If that prefix
        // accounts for all retained calls, its tail is empty. Count completed
        // and dispatched calls too; holes keep the conservative node bound.
        let slots = if self.host_slot_search_start == self.pending_host_count {
            self.pending_host_count
        } else {
            self.active_nodes.min(PENDING_REQUESTS)
        };
        let cursor = if self.host_request_cursor < slots {
            self.host_request_cursor
        } else {
            0
        };
        for offset in 0..slots {
            #[cfg(test)]
            {
                let mut counts = self.scan_counts.get();
                counts.dispatch_pending_slots += 1;
                self.scan_counts.set(counts);
            }
            let slot = (cursor + offset) % slots;
            let Some(pending) = self.pending_host_calls[slot].as_mut() else {
                continue;
            };
            if pending.dispatched {
                continue;
            }
            remaining -= 1;
            if !accepts(&pending.request) {
                if remaining == 0 {
                    break;
                }
                continue;
            }
            pending.dispatched = true;
            self.undispatched_host_count -= 1;
            self.host_request_cursor = (slot + 1) % PENDING_REQUESTS;
            return Some(pending.request);
        }
        None
    }
}
