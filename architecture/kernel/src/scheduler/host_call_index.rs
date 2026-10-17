//! Borrowed per-node lookup into the canonical pending request table.
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
    pub(super) fn indexed_host_call(&self, node: usize) -> Option<&PendingHostCall> {
        let slot = self.pending_host_slots[node]?;
        self.pending_host_calls[usize::try_from(slot.get() - 1).ok()?].as_ref()
    }
}

pub(super) fn encode_slot(slot: usize) -> Result<core::num::NonZeroU32, SchedulerError> {
    slot.checked_add(1)
        .and_then(|slot| u32::try_from(slot).ok())
        .and_then(core::num::NonZeroU32::new)
        .ok_or(SchedulerError::InvalidPlan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_represents_the_entire_u16_node_cardinality_without_a_sentinel_collision() {
        assert_eq!(encode_slot(0).unwrap().get(), 1);
        assert_eq!(encode_slot(usize::from(u16::MAX)).unwrap().get(), 65_536);
        assert_eq!(encode_slot(usize::MAX), Err(SchedulerError::InvalidPlan));
        assert_eq!(core::mem::size_of::<Option<core::num::NonZeroU32>>(), 4);
    }
}
