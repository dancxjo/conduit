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
    pub(super) fn preflight_payload_output(
        &self,
        spec: CordSpec,
        state: &CordState,
        bytes: u32,
    ) -> Result<(), SchedulerError> {
        if state.len >= spec.item_capacity
            && (spec.pressure_policy != AssignedPressurePolicy::CoalesceLatest || state.len == 0)
        {
            return Err(SchedulerError::QueueCapacityExceeded);
        }
        if bytes > spec.maximum_value_bytes {
            return Err(SchedulerError::SemanticValueBoundExceeded);
        }
        if bytes > self.admission_maximum(spec, state)? {
            return Err(SchedulerError::QueueByteCapacityExceeded);
        }
        Ok(())
    }

    fn ensure_output_value_envelope(
        &self,
        node: usize,
        port: PortId,
        bytes: u32,
    ) -> Result<(), SchedulerError> {
        for target in self.routes.route(NodeId(as_u16(node)?), port)? {
            let spec = self.cord_specs[usize::from(target.cord.0)];
            if spec.track == AssignedConnectionTrack::Payload && bytes > spec.maximum_value_bytes {
                return Err(SchedulerError::SemanticValueBoundExceeded);
            }
        }
        Ok(())
    }

    pub(super) fn materialize_step_output(
        &mut self,
        node: usize,
        io: &mut StepIo<PORTS>,
    ) -> Result<[Option<ValueRef>; PORTS], SchedulerError> {
        let mut generated = [None; PORTS];
        let mut prepared = [None; PORTS];
        // Validate every prepared value before storing any: a later semantic
        // violation must not be obscured by earlier value-store pressure.
        for (index, length) in io.prepared_outputs.iter().enumerate() {
            let Some(length) = length else {
                continue;
            };
            let port = PortId(as_u16(index)?);
            let bytes = self.drivers[node]
                .prepared_output(port)
                .ok_or(SchedulerError::InvalidPlan)?;
            if u32::try_from(bytes.len()).ok() != Some(*length) {
                return Err(SchedulerError::InvalidPlan);
            }
            if io.outputs[index].is_some() {
                return Err(SchedulerError::InvalidPortAccess);
            }
            self.ensure_output_value_envelope(node, port, *length)?;
            prepared[index] = Some(bytes);
        }
        if let Some((port, value)) = io.canonical_output {
            self.ensure_output_value_envelope(node, port, value.as_slice().len() as u32)?;
            let value =
                derived_value::materialize(&mut self.values, io.canonical_output, &mut io.outputs)?;
            generated[usize::from(port.0)] = value;
            return Ok(generated);
        }
        for (index, bytes) in prepared.into_iter().enumerate() {
            let Some(bytes) = bytes else {
                continue;
            };
            match self.values.store(bytes) {
                Ok(value) => {
                    generated[index] = Some(value);
                    io.outputs[index] = Some(value);
                }
                Err(error) => {
                    for (index, value) in generated.into_iter().enumerate() {
                        if let Some(value) = value {
                            self.values.release(value)?;
                            io.outputs[index] = None;
                        }
                    }
                    return Err(error.into());
                }
            }
        }
        Ok(generated)
    }
}
