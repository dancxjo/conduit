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
        if self.pending_host_calls.iter().flatten().any(|pending| {
            pending.request.input.value == value
                || pending
                    .completion
                    .and_then(|outcome| outcome.output)
                    .map(|output| output.value)
                    == Some(value)
        }) || self
            .queue_slots
            .iter()
            .flatten()
            .any(|queued| *queued == value)
        {
            return Err(HostValueDiscardRefusal::ValueOwned);
        }
        self.values
            .release(value)
            .map_err(HostValueDiscardRefusal::Storage)
    }
}
