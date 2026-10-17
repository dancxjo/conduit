//! Storage and exact ownership of values crossing the Host Call boundary.
use super::*;

/// Only unconsumed, unqueued host values may be discarded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostValueDiscardRefusal {
    ValueOwned,
    Storage(StorageError),
}

impl From<HostValueDiscardRefusal> for SchedulerError {
    fn from(error: HostValueDiscardRefusal) -> Self {
        match error {
            HostValueDiscardRefusal::ValueOwned => Self::ValueOwnershipViolation,
            HostValueDiscardRefusal::Storage(error) => Self::Storage(error),
        }
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
    D: StepBack<PORTS>,
    S: ValueStorage,
    E: SignSink,
{
    pub fn store_host_value(&mut self, bytes: &[u8]) -> Result<ValueRef, SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        Ok(self.values.store(bytes)?)
    }

    pub fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        Ok(self.values.get(value)?)
    }

    pub fn discard_host_value(&mut self, value: ValueRef) -> Result<(), HostValueDiscardRefusal> {
        let pending = self
            .pending_host_calls
            .iter()
            .take(if self.pending_host_count == 0 {
                0
            } else {
                PENDING_REQUESTS
            });
        #[cfg(test)]
        let pending = pending.inspect(|_| {
            let mut counts = self.scan_counts.get();
            counts.value_owner_pending_slots += 1;
            self.scan_counts.set(counts);
        });
        if pending.flatten().any(|pending| {
            pending.request.input.value == value
                || pending
                    .completion
                    .and_then(|outcome| outcome.output)
                    .map(|output| output.value)
                    == Some(value)
        }) || self.queue_owns_host_value(value)
        {
            return Err(HostValueDiscardRefusal::ValueOwned);
        }
        self.values
            .release(value)
            .map_err(HostValueDiscardRefusal::Storage)
    }

    fn queue_owns_host_value(&self, value: ValueRef) -> bool {
        self.cords[..self.active_cords]
            .iter()
            .enumerate()
            .any(|(cord, state)| {
                let spec = self.cord_specs[cord];
                (0..state.len).any(|offset| {
                    // Preserve ring order without overflowing a u16 head+offset.
                    let until_wrap = spec.item_capacity - state.head;
                    let position = if offset < until_wrap {
                        state.head + offset
                    } else {
                        offset - until_wrap
                    };
                    let slot = usize::from(spec.slot_start) + usize::from(position);
                    #[cfg(test)]
                    {
                        let mut counts = self.scan_counts.get();
                        counts.value_owner_queue_slots += 1;
                        self.scan_counts.set(counts);
                    }
                    self.queue_slots[slot] == Some(value)
                })
            })
    }
}
