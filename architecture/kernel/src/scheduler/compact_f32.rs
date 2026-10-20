//! Pre-Play compact-lane binding over unchanged logical transactions.
use super::*;
use alloc::{boxed::Box, vec::Vec};

// Only selected hosted profiles allocate this fixed-capacity sidecar.
pub(super) struct CompactF32Bindings<const N: usize, const P: usize> {
    inputs: [[bool; P]; N],
    outputs: [Option<(PortId, CompactF32Layout)>; N],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactF32Layout {
    pub lanes: u16,
    pub logical_bytes: u32,
}
impl CompactF32Layout {
    pub(super) fn physical_bytes(self) -> Result<u32, SchedulerError> {
        let bytes = u32::from(self.lanes) * 4;
        if bytes == 0 || bytes > self.logical_bytes {
            return Err(SchedulerError::InvalidPlan);
        }
        Ok(bytes)
    }
}
impl<const P: usize> StepInputBytes<'_, P> {
    pub fn compact_f32(&self, port: PortId) -> Option<&[u8]> {
        self.compact_f32
            .and_then(|inputs| inputs.get(usize::from(port.0)))
            .copied()
            .flatten()
    }
}
impl<const P: usize> StepIo<P> {
    pub fn compact_f32_output(&self, port: PortId) -> Option<CompactF32Layout> {
        self.compact_f32_output
            .filter(|(p, _)| *p == port)
            .map(|(_, layout)| layout)
    }
    /// Publish the exact prepared Back buffer through the existing commit path.
    pub fn send_compact_f32(&mut self, port: PortId) -> Result<(), SchedulerError> {
        self.consume_fuel(1)?;
        let Some(layout) = self.compact_f32_output(port) else {
            return self.fail(SchedulerError::InvalidPortAccess);
        };
        let index = usize::from(port.0);
        if self
            .output_maximum_bytes
            .get(index)
            .copied()
            .flatten()
            .is_none_or(|n| n < layout.logical_bytes)
            || self.outputs.get(index).is_none_or(Option::is_some)
            || self.canonical_output.is_some()
            || self.prepared_output.is_some()
        {
            return self.fail(SchedulerError::OutputBlocked);
        }
        self.prepared_output = Some((port, PreparedStepValue::CompactF32(layout)));
        Ok(())
    }
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
    pub(super) fn has_compact_f32_input(&self, node: usize, port: usize) -> bool {
        self.compact_f32_bindings
            .as_deref()
            .is_some_and(|b| b.inputs[node][port])
    }
    pub(super) fn has_compact_f32_inputs(&self, node: usize) -> bool {
        self.compact_f32_bindings
            .as_deref()
            .is_some_and(|b| b.inputs[node].iter().any(|p| *p))
    }
    pub(super) fn compact_f32_output_binding(
        &self,
        node: usize,
    ) -> Option<(PortId, CompactF32Layout)> {
        self.compact_f32_bindings
            .as_deref()
            .and_then(|b| b.outputs[node])
    }
    pub(super) fn has_compact_f32_node(&self, node: usize) -> bool {
        self.has_compact_f32_inputs(node) || self.compact_f32_output_binding(node).is_some()
    }
    pub(super) fn has_compact_f32_outputs(&self) -> bool {
        self.compact_f32_bindings
            .as_deref()
            .is_some_and(|b| b.outputs.iter().any(Option::is_some))
    }
    /// Additional prepared heap backing. The unselected profile owns none.
    pub fn compact_f32_binding_bytes(&self) -> usize {
        self.compact_f32_bindings
            .as_ref()
            .map_or(0, |_| core::mem::size_of::<CompactF32Bindings<N, P>>())
    }
    /// Exact Host/Plan preparation must prove purity, type, temporal and resource
    /// eligibility. This binder checks physical layout, local DAG topology,
    /// atomic fanout, finite storage and debugger/Host Call fences. External
    /// outputs stay canonical. Physical continuation follows the existing kernel
    /// Step loop; every logical Step, queue transaction and Sign still runs.
    pub fn bind_compact_f32_dag(&mut self, nodes: &[NodeId]) -> Result<(), SchedulerError> {
        if !(2..=32).contains(&nodes.len())
            || self.decisions != 0
            || self.cancelled
            || !self.debug_control.is_clear()
            || self.signs.requires_canonical_debug_values()
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let capacity = nodes
            .len()
            .checked_mul(P)
            .ok_or(SchedulerError::InvalidPlan)?;
        let mut outputs = Vec::with_capacity(nodes.len());
        let mut inputs = Vec::with_capacity(capacity);
        for (ordinal, node) in nodes.iter().enumerate() {
            let n = usize::from(node.0);
            if n >= self.active_nodes
                || nodes[..ordinal].contains(node)
                || self.native_input_ports[n].is_some()
                || self.native_output_ports[n].is_some()
                || self.has_compact_f32_node(n)
                || self.region_successors[n].is_some()
                || self.region_successors.contains(&Some(*node))
                || self.terminal_transductions[n].iter().any(Option::is_some)
                || self.host_bindings.as_ref().is_some_and(|table| {
                    table.bindings.iter().enumerate().any(|(slot, b)| {
                        b.is_some() && slot / usize::from(table.maximum_gears_per_node) == n
                    })
                })
            {
                return Err(SchedulerError::InvalidPlan);
            }
            let (port, layout) = self.drivers[n]
                .compact_f32_output()
                .ok_or(SchedulerError::InvalidPlan)?;
            layout.physical_bytes()?;
            if usize::from(port.0) >= P {
                return Err(SchedulerError::InvalidPlan);
            }
            for (p, cord) in self.node_specs[n].input_cords.iter().enumerate() {
                if cord.is_some() {
                    self.drivers[n]
                        .compact_f32_input(PortId(as_u16(p)?))
                        .ok_or(SchedulerError::InvalidPlan)?
                        .physical_bytes()?;
                }
            }
            let outgoing = || {
                self.cord_specs[..self.active_cords]
                    .iter()
                    .filter(|c| c.source_local().is_some_and(|(source, _)| source == *node))
            };
            if outgoing().any(|c| c.source_local().unwrap().1 != port) {
                return Err(SchedulerError::InvalidPlan);
            }
            if outgoing().next().is_none()
                || outgoing().any(|c| {
                    c.sink_local()
                        .is_none_or(|(sink, _)| !nodes.contains(&sink))
                })
            {
                continue;
            }
            for cord in outgoing() {
                let (sink, input) = cord.sink_local().ok_or(SchedulerError::InvalidPlan)?;
                let sink_index = usize::from(sink.0);
                if nodes
                    .iter()
                    .position(|n| *n == sink)
                    .is_none_or(|i| i <= ordinal)
                    || cord.track != AssignedConnectionTrack::Payload
                    || cord.pressure_policy != AssignedPressurePolicy::PreserveOrder
                    || cord.maximum_value_bytes != layout.logical_bytes
                    || cord.byte_capacity < layout.logical_bytes
                    || self.drivers[sink_index].compact_f32_input(input) != Some(layout)
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                if inputs.len() == capacity {
                    return Err(SchedulerError::InvalidPlan);
                }
                inputs.push((sink, input));
            }
            outputs.push((*node, port, layout));
        }
        if outputs.is_empty()
            || nodes.iter().any(|node| {
                !outputs.iter().any(|(n, _, _)| n == node) && !inputs.iter().any(|(n, _)| n == node)
            })
        {
            return Err(SchedulerError::InvalidPlan);
        }
        // Reachability and order are proved before any binding is published.
        let mut reached = [false; 32];
        reached[0] = true;
        for _ in 0..nodes.len() {
            for cord in &self.cord_specs[..self.active_cords] {
                let (Some((source, port)), Some((sink, input))) =
                    (cord.source_local(), cord.sink_local())
                else {
                    continue;
                };
                if !outputs.iter().any(|(n, p, _)| *n == source && *p == port)
                    || !inputs.contains(&(sink, input))
                {
                    continue;
                }
                let a = nodes.iter().position(|n| *n == source).unwrap();
                let b = nodes.iter().position(|n| *n == sink).unwrap();
                if reached[a] || reached[b] {
                    reached[a] = true;
                    reached[b] = true;
                }
            }
        }
        if reached[..nodes.len()].iter().any(|b| !*b) {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut items = 1u32;
        let mut bytes = 0u32;
        let mut maximum = 0u32;
        for cord in &self.cord_specs[..self.active_cords] {
            let (Some((source, port)), Some((sink, input))) =
                (cord.source_local(), cord.sink_local())
            else {
                continue;
            };
            let new = outputs
                .iter()
                .find(|(n, p, _)| *n == source && *p == port)
                .map(|(_, _, l)| *l)
                .filter(|_| inputs.contains(&(sink, input)));
            let old = self
                .compact_f32_output_binding(usize::from(source.0))
                .filter(|(p, _)| *p == port)
                .map(|(_, l)| l)
                .filter(|_| self.has_compact_f32_input(usize::from(sink.0), usize::from(input.0)));
            if let Some(layout) = new.or(old) {
                items = items
                    .checked_add(u32::from(cord.item_capacity))
                    .ok_or(SchedulerError::InvalidPlan)?;
                bytes = bytes
                    .checked_add(
                        u32::from(cord.item_capacity)
                            .checked_mul(layout.logical_bytes)
                            .ok_or(SchedulerError::InvalidPlan)?,
                    )
                    .ok_or(SchedulerError::InvalidPlan)?;
                maximum = maximum.max(layout.logical_bytes);
            }
        }
        bytes = bytes
            .checked_add(maximum)
            .ok_or(SchedulerError::InvalidPlan)?;
        let (available, quota) = self.values.compact_f32_available(maximum);
        if u32::from(available) < items || quota < bytes {
            return Err(SchedulerError::InvalidPlan);
        }
        let prepared = self.compact_f32_bindings.is_none().then(|| {
            Box::new(CompactF32Bindings {
                inputs: [[false; P]; N],
                outputs: [None; N],
            })
        });
        self.bind_execution_dag(nodes)?;
        if let Some(bindings) = prepared {
            self.compact_f32_bindings = Some(bindings);
        }
        let bindings = self
            .compact_f32_bindings
            .as_deref_mut()
            .expect("preflight prepared compact bindings");
        for (node, port, layout) in outputs {
            bindings.outputs[usize::from(node.0)] = Some((port, layout));
        }
        for (node, port) in inputs {
            bindings.inputs[usize::from(node.0)][usize::from(port.0)] = true;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ignored_invalid_publication_still_faults_the_transaction() {
        let mut io = StepIo::single_source(16, 8, None);
        assert_eq!(
            io.send_compact_f32(PortId(0)),
            Err(SchedulerError::InvalidPortAccess)
        );
        assert_eq!(io.fault, Some(SchedulerError::InvalidPortAccess));
    }
}
