//! Opt-in physical native scalar transport through ordinary logical Steps.
use super::*;
impl<const P: usize> StepInputBytes<'_, P> {
    pub fn native_u64(&self, port: PortId) -> Option<u64> {
        self.native_u64
            .and_then(|values| values.get(usize::from(port.0)))
            .copied()
            .flatten()
    }
}
impl<const P: usize> StepIo<P> {
    pub fn native_u64_output(&self, port: PortId) -> bool {
        self.native_output_port == Some(port)
    }
    pub fn send_native_u64(&mut self, port: PortId, value: u64) -> Result<(), SchedulerError> {
        self.consume_fuel(1)?;
        let index = usize::from(port.0);
        if self.native_output_port != Some(port)
            || self
                .output_maximum_bytes
                .get(index)
                .copied()
                .flatten()
                .is_none_or(|n| n < 8)
            || self.outputs.get(index).is_none_or(Option::is_some)
            || self.canonical_output.is_some()
            || self.prepared_output.is_some()
        {
            return self.fail(SchedulerError::OutputBlocked);
        }
        self.prepared_output = Some((port, PreparedStepValue::NativeU64(value)));
        Ok(())
    }
}
pub(super) fn observed_bytes<S: ValueStorage>(
    store: &S,
    value: ValueRef,
    native: bool,
) -> Result<Option<&[u8]>, SchedulerError> {
    if native && (store.native_u64(value)?.is_some() || store.compact_f32(value)?.is_some()) {
        return Ok(None);
    }
    Ok(Some(store.get(value)?))
}
impl<
        D,
        S,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const A: usize,
    > FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>
where
    D: StepBack<P>,
    S: ValueStorage,
    E: SignSink,
{
    /// Bind an admitted pure unary U64 chain. This proves physical envelopes,
    /// not semantic purity: exact Plan/Back/type eligibility remains the Host's
    /// obligation. Native values never escape the internal local chain. The
    /// existing Step/queue/commit/Sign transactions remain authoritative.
    pub fn bind_native_u64_chain(&mut self, nodes: &[NodeId]) -> Result<(), SchedulerError> {
        if !(2..=32).contains(&nodes.len())
            || self.decisions != 0
            || self.cancelled
            || !self.debug_control.is_clear()
            || self.signs.requires_canonical_debug_values()
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut reservations = 1u32;
        // All disjoint regions share this store. Reserve their queued native
        // values together, plus one not-yet-published generated value.
        for cord in &self.cord_specs[..self.active_cords] {
            if let (Some((source, output)), Some((sink, input))) =
                (cord.source_local(), cord.sink_local())
            {
                if self.native_output_ports[usize::from(source.0)] == Some(output)
                    && self.native_input_ports[usize::from(sink.0)] == Some(input)
                {
                    reservations = reservations
                        .checked_add(u32::from(cord.item_capacity))
                        .ok_or(SchedulerError::InvalidPlan)?;
                }
            }
        }
        for node in nodes {
            let n = usize::from(node.0);
            if n >= self.active_nodes
                || self.native_input_ports[n].is_some()
                || self.native_output_ports[n].is_some()
                || self.has_compact_f32_node(n)
                || self.terminal_transductions[n].iter().any(Option::is_some)
            {
                return Err(SchedulerError::InvalidPlan);
            }
            let (input, output) = self.drivers[n]
                .native_u64_ports()
                .ok_or(SchedulerError::InvalidPlan)?;
            if usize::from(input.0) >= P
                || usize::from(output.0) >= P
                || self.node_specs[n].input_cords.iter().flatten().count() != 1
                || self.node_specs[n].input_cords[usize::from(input.0)].is_none()
            {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        for pair in nodes.windows(2) {
            let output = self.drivers[usize::from(pair[0].0)]
                .native_u64_ports()
                .unwrap()
                .1;
            let input = self.drivers[usize::from(pair[1].0)]
                .native_u64_ports()
                .unwrap()
                .0;
            let cord = self.cord_specs[..self.active_cords]
                .iter()
                .find(|c| {
                    c.source == CordEndpoint::local(pair[0], output)
                        && c.sink == CordEndpoint::local(pair[1], input)
                })
                .ok_or(SchedulerError::InvalidPlan)?;
            if cord.track != AssignedConnectionTrack::Payload
                || cord.maximum_value_bytes != 8
                || cord.byte_capacity < 8
                || cord.pressure_policy != AssignedPressurePolicy::PreserveOrder
            {
                return Err(SchedulerError::InvalidPlan);
            }
            reservations = reservations
                .checked_add(u32::from(cord.item_capacity))
                .ok_or(SchedulerError::InvalidPlan)?;
        }
        let (items, bytes) = self.values.native_u64_available();
        if u32::from(items) < reservations
            || bytes
                < reservations
                    .checked_mul(8)
                    .ok_or(SchedulerError::InvalidPlan)?
        {
            return Err(SchedulerError::InvalidPlan);
        }
        // The existing binder preflights complete topology/Host/overlap before
        // publishing any successor. Every later write below is infallible.
        self.bind_execution_chain(nodes)?;
        for pair in nodes.windows(2) {
            self.native_output_ports[usize::from(pair[0].0)] = Some(
                self.drivers[usize::from(pair[0].0)]
                    .native_u64_ports()
                    .unwrap()
                    .1,
            );
            self.native_input_ports[usize::from(pair[1].0)] = Some(
                self.drivers[usize::from(pair[1].0)]
                    .native_u64_ports()
                    .unwrap()
                    .0,
            );
        }
        Ok(())
    }
}
