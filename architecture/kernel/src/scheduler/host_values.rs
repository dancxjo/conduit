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
        self.values
            .reference_count(value)
            .map_err(HostValueDiscardRefusal::Storage)?;
        if self
            .pending_owns_host_value(value)
            .map_err(HostValueDiscardRefusal::Storage)?
            || self
                .queue_owns_host_value(value)
                .map_err(HostValueDiscardRefusal::Storage)?
        {
            return Err(HostValueDiscardRefusal::ValueOwned);
        }
        self.values
            .release(value)
            .map_err(HostValueDiscardRefusal::Storage)
    }

    fn pending_owns_host_value(&self, value: ValueRef) -> Result<bool, StorageError> {
        let mut remaining = self.pending_host_count;
        for entry in &self.pending_host_calls {
            // Occupancy includes completions awaiting consumption. Once all
            // live entries were visited, the remaining reserved tail is empty.
            if remaining == 0 {
                break;
            }
            #[cfg(test)]
            {
                let mut counts = self.scan_counts.get();
                counts.value_owner_pending_slots += 1;
                self.scan_counts.set(counts);
            }
            let Some(pending) = entry else {
                continue;
            };
            remaining -= 1;
            if pending.request.input.value == value
                || pending
                    .completion
                    .map(|outcome| outcome.resolve(&self.values))
                    .transpose()?
                    .and_then(|outcome| outcome.output)
                    .map(|output| output.value)
                    == Some(value)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn queue_owns_host_value(&self, value: ValueRef) -> Result<bool, StorageError> {
        for (cord, state) in self.cords[..self.active_cords].iter().enumerate() {
            let spec = self.cord_specs[cord];
            for offset in 0..state.len {
                let slot = super::queue_index::queue_slot(spec, state.head, offset);
                #[cfg(test)]
                {
                    let mut counts = self.scan_counts.get();
                    counts.value_owner_queue_slots += 1;
                    self.scan_counts.set(counts);
                }
                if self.queue_slots[slot]
                    .map(|key| key.resolve(&self.values))
                    .transpose()?
                    == Some(value)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
