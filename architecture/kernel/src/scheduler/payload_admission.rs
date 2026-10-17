//! Queue pressure and semantic value admission at the remote payload boundary.
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
    /// Zero byte allowance does not distinguish a full queue from a queue
    /// that can admit an empty payload. Check item pressure independently.
    pub(super) fn payload_admissible(
        &self,
        spec: CordSpec,
        state: &CordState,
        byte_len: u32,
    ) -> Result<bool, SchedulerError> {
        if byte_len > spec.maximum_value_bytes {
            return Err(SchedulerError::SemanticValueBoundExceeded);
        }
        if state.len >= spec.item_capacity
            && (spec.pressure_policy != AssignedPressurePolicy::CoalesceLatest || state.len == 0)
        {
            return Ok(false);
        }
        Ok(byte_len <= self.admission_maximum(spec, state)?)
    }

    pub(super) fn admission_maximum(
        &self,
        spec: CordSpec,
        state: &CordState,
    ) -> Result<u32, SchedulerError> {
        if state.len < spec.item_capacity {
            return Ok(spec.byte_capacity.saturating_sub(state.queued_bytes));
        }
        if spec.pressure_policy != AssignedPressurePolicy::CoalesceLatest || state.len == 0 {
            return Ok(0);
        }
        let slot = queue_slot(spec, state.head, state.len - 1);
        let newest = self.queue_slots[slot].ok_or(SchedulerError::InvalidPlan)?;
        Ok(spec
            .byte_capacity
            .saturating_sub(state.queued_bytes.saturating_sub(newest.byte_len)))
    }
}
