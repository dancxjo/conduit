//! Queue pressure and semantic value admission at the remote payload boundary.
use super::*;

pub(super) struct GeneratedStepOutput {
    #[cfg(feature = "alloc")]
    value: ValueRef,
    #[cfg(feature = "alloc")]
    owned: Option<(PortId, crate::PendingCompactF32Publication)>,
}
impl GeneratedStepOutput {
    fn copied(value: ValueRef) -> Self {
        #[cfg(not(feature = "alloc"))]
        let _ = value;
        Self {
            #[cfg(feature = "alloc")]
            value,
            #[cfg(feature = "alloc")]
            owned: None,
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
        let newest = self.queue_slots[slot]
            .ok_or(SchedulerError::InvalidPlan)?
            .resolve(&self.values)?;
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
    ) -> Result<Option<GeneratedStepOutput>, SchedulerError> {
        // Validate every declared semantic envelope before any storage effect.
        for (index, output) in io.prepared_outputs.iter().enumerate() {
            if let Some(output) = output {
                self.ensure_output_value_envelope(
                    node,
                    PortId(as_u16(index)?),
                    output.logical_bytes(),
                )?;
            }
        }
        // Current immutable binding admits at most one owned compact output per
        // node. Copied outputs are already recorded in io.outputs; do not create
        // a PORTS-sized array of heavyweight pending ownership tokens.
        #[allow(unused_mut)]
        let mut owned = None;
        for index in 0..PORTS {
            let Some(output) = io.prepared_outputs[index] else {
                continue;
            };
            match self.materialize_prepared_output(node, io, PortId(as_u16(index)?), output) {
                Ok(value) => {
                    #[cfg(feature = "alloc")]
                    if value.as_ref().is_some_and(|value| value.owned.is_some()) {
                        owned = value;
                    }
                    #[cfg(not(feature = "alloc"))]
                    let _ = value;
                }
                Err(error) => {
                    self.rollback_generated_output(node, owned, io)?;
                    return Err(error);
                }
            }
        }
        if let Some((port, value)) = io.canonical_output {
            self.ensure_output_value_envelope(node, port, value.as_slice().len() as u32)?;
            // Canonical and prepared staging remain mutually exclusive.
            return derived_value::materialize(
                &mut self.values,
                io.canonical_output,
                &mut io.outputs,
            )
            .map(|value| value.map(GeneratedStepOutput::copied));
        }
        Ok(owned)
    }
    fn materialize_prepared_output(
        &mut self,
        node: usize,
        io: &mut StepIo<PORTS>,
        port: PortId,
        output: PreparedStepValue,
    ) -> Result<Option<GeneratedStepOutput>, SchedulerError> {
        let declared_len = output.logical_bytes();
        #[cfg(feature = "alloc")]
        if let PreparedStepValue::CompactCanonical(logical) = output {
            let (bound_port, layout) = self
                .compact_canonical_output_binding(node)
                .ok_or(SchedulerError::InvalidPlan)?;
            if bound_port != port {
                return Err(SchedulerError::InvalidPlan);
            }
            self.ensure_output_value_envelope(node, port, logical)?;
            let bytes = self.drivers[node]
                .prepared_compact_canonical_output(port)
                .ok_or(SchedulerError::InvalidPlan)?;
            let value = self
                .values
                .store_compact_canonical(layout, bytes, logical)?;
            let slot = io
                .outputs
                .get_mut(usize::from(port.0))
                .ok_or(SchedulerError::InvalidPortAccess)?;
            if slot.is_some() {
                self.values.release(value)?;
                return Err(SchedulerError::InvalidPortAccess);
            }
            *slot = Some(value);
            return Ok(Some(GeneratedStepOutput::copied(value)));
        }
        #[cfg(feature = "alloc")]
        if let PreparedStepValue::NativeU64(value) = output {
            if self.native_output_ports[node] != Some(port) {
                return Err(SchedulerError::InvalidPlan);
            }
            self.ensure_output_value_envelope(node, port, declared_len)?;
            let value = self.values.store_native_u64(value)?;
            let slot = io
                .outputs
                .get_mut(usize::from(port.0))
                .ok_or(SchedulerError::InvalidPortAccess)?;
            if slot.is_some() {
                self.values.release(value)?;
                return Err(SchedulerError::InvalidPortAccess);
            }
            *slot = Some(value);
            return Ok(Some(GeneratedStepOutput::copied(value)));
        }
        #[cfg(feature = "alloc")]
        if let PreparedStepValue::CompactF32(layout) = output {
            if self.compact_f32_output_binding(node) != Some((port, layout)) {
                return Err(SchedulerError::InvalidPlan);
            }
            self.ensure_output_value_envelope(node, port, declared_len)?;
            if let Some(owner) = self
                .compact_f32_bindings
                .as_deref()
                .and_then(|bindings| bindings.owned.as_deref())
                .and_then(|owners| owners[node].as_ref())
            {
                let slot = io
                    .outputs
                    .get_mut(usize::from(port.0))
                    .ok_or(SchedulerError::InvalidPortAccess)?;
                if slot.is_some() {
                    return Err(SchedulerError::InvalidPortAccess);
                }
                let bytes = self.drivers[node]
                    .prepared_compact_f32_output_mut(port)
                    .ok_or(SchedulerError::InvalidPlan)?;
                let pending = self.values.swap_owned_compact_f32(owner, bytes)?;
                let value = pending.value()?;
                *slot = Some(value);
                return Ok(Some(GeneratedStepOutput {
                    value,
                    owned: Some((port, pending)),
                }));
            }
            let bytes = self.drivers[node]
                .prepared_output(port)
                .ok_or(SchedulerError::InvalidPlan)?;
            if u32::try_from(bytes.len()).ok() != Some(layout.physical_bytes()?) {
                return Err(SchedulerError::InvalidPlan);
            }
            let value = self.values.store_compact_f32(bytes, declared_len)?;
            let slot = io
                .outputs
                .get_mut(usize::from(port.0))
                .ok_or(SchedulerError::InvalidPortAccess)?;
            if slot.is_some() {
                self.values.release(value)?;
                return Err(SchedulerError::InvalidPortAccess);
            }
            *slot = Some(value);
            return Ok(Some(GeneratedStepOutput::copied(value)));
        }
        if self.drivers[node].encodes_prepared_output(port) {
            self.ensure_output_value_envelope(node, port, declared_len)?;
            let output = io
                .outputs
                .get_mut(usize::from(port.0))
                .ok_or(SchedulerError::InvalidPortAccess)?;
            if output.is_some() {
                return Err(SchedulerError::InvalidPortAccess);
            }
            let declared =
                usize::try_from(declared_len).map_err(|_| StorageError::ValueTooLarge)?;
            let driver = &self.drivers[node];
            let value = self.values.store_prepared_encoded(declared, &mut |bytes| {
                let length = driver.encode_prepared_output(port, bytes)?;
                if length != declared {
                    return Err(StorageError::ValueTooLarge);
                }
                Ok(length)
            })?;
            *output = Some(value);
            return Ok(Some(GeneratedStepOutput::copied(value)));
        }
        let bytes = self.drivers[node]
            .prepared_output(port)
            .ok_or(SchedulerError::InvalidPlan)?;
        if u32::try_from(bytes.len()).ok() != Some(declared_len) {
            return Err(SchedulerError::InvalidPlan);
        }
        self.ensure_output_value_envelope(node, port, declared_len)?;
        let value = self.values.store(bytes)?;
        let output = io
            .outputs
            .get_mut(usize::from(port.0))
            .ok_or(SchedulerError::InvalidPortAccess)?;
        if output.is_some() {
            self.values.release(value)?;
            return Err(SchedulerError::InvalidPortAccess);
        }
        *output = Some(value);
        Ok(Some(GeneratedStepOutput::copied(value)))
    }
    pub(super) fn rollback_generated_output(
        &mut self,
        node: usize,
        generated: Option<GeneratedStepOutput>,
        io: &StepIo<PORTS>,
    ) -> Result<(), SchedulerError> {
        #[cfg(feature = "alloc")]
        let owned_port = generated
            .as_ref()
            .and_then(|value| value.owned.as_ref().map(|(port, _)| *port));
        for index in (0..PORTS).rev() {
            let port = PortId(as_u16(index)?);
            #[cfg(feature = "alloc")]
            if owned_port == Some(port) {
                continue;
            }
            if io.prepared_outputs[index].is_some()
                || io
                    .canonical_output
                    .is_some_and(|(canonical, _)| canonical == port)
            {
                if let Some(value) = io.outputs[index] {
                    let _ = self.values.release(value);
                }
            }
        }
        // Only owned transfer needs a token; copied releases used the existing
        // staged output slots above. Restore the original producer allocation.
        #[cfg(feature = "alloc")]
        if generated
            .as_ref()
            .is_some_and(|value| value.owned.is_some())
        {
            return self.rollback_one_generated_output(node, generated);
        }
        let _ = (node, generated);
        Ok(())
    }
    pub(super) fn commit_generated_output(
        &mut self,
        node: usize,
        generated: Option<GeneratedStepOutput>,
    ) -> Result<(), SchedulerError> {
        self.commit_one_generated_output(node, generated)
    }
    #[cfg(feature = "alloc")]
    fn rollback_one_generated_output(
        &mut self,
        node: usize,
        generated: Option<GeneratedStepOutput>,
    ) -> Result<(), SchedulerError> {
        #[allow(unused_mut)]
        let Some(mut generated) = generated
        else {
            return Ok(());
        };
        #[cfg(feature = "alloc")]
        if let Some((port, pending)) = &mut generated.owned {
            let owner = self
                .compact_f32_bindings
                .as_deref()
                .and_then(|bindings| bindings.owned.as_deref())
                .and_then(|owners| owners[node].as_ref())
                .ok_or(SchedulerError::InvalidPlan)?;
            let bytes = self.drivers[node]
                .prepared_compact_f32_output_mut(*port)
                .ok_or(SchedulerError::InvalidPlan)?;
            self.values
                .restore_owned_compact_f32(owner, pending, bytes)?;
            return Ok(());
        }
        #[cfg(not(feature = "alloc"))]
        let _ = node;
        let _ = self.values.release(generated.value);
        Ok(())
    }
    fn commit_one_generated_output(
        &mut self,
        node: usize,
        generated: Option<GeneratedStepOutput>,
    ) -> Result<(), SchedulerError> {
        #[cfg(feature = "alloc")]
        if let Some(GeneratedStepOutput {
            owned: Some((_, mut pending)),
            ..
        }) = generated
        {
            let owner = self
                .compact_f32_bindings
                .as_deref()
                .and_then(|bindings| bindings.owned.as_deref())
                .and_then(|owners| owners[node].as_ref())
                .ok_or(SchedulerError::InvalidPlan)?;
            self.values.commit_owned_compact_f32(owner, &mut pending)?;
        }
        #[cfg(not(feature = "alloc"))]
        let _ = (node, generated);
        Ok(())
    }
}
