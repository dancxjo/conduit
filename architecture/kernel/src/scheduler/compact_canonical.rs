//! Explicit trusted local lossless representation over unchanged Cord truth.
use super::*;
#[cfg(feature = "alloc")]
use crate::CompactCanonicalLayout;

#[cfg(feature = "alloc")]
pub(super) struct Binding {
    pub(super) source: (NodeId, PortId),
    pub(super) sink: (NodeId, PortId),
    pub(super) layout: CompactCanonicalLayout,
}
impl<const P: usize> StepInputBytes<'_, P> {
    /// Available only on the exact explicitly bound representation-aware input.
    /// Generic canonical input() remains None for this physical representation.
    pub fn compact_canonical(&self, port: PortId) -> Option<&[u8]> {
        #[cfg(feature = "alloc")]
        {
            self.compact_canonical
                .and_then(|v| v.get(usize::from(port.0)))
                .copied()
                .flatten()
        }
        #[cfg(not(feature = "alloc"))]
        {
            let _ = port;
            None
        }
    }
}
impl<const P: usize> StepIo<P> {
    pub fn compact_canonical_output(&self, port: PortId) -> bool {
        #[cfg(feature = "alloc")]
        {
            self.compact_canonical_output == Some(port)
        }
        #[cfg(not(feature = "alloc"))]
        {
            let _ = port;
            false
        }
    }
    /// Logical length is the exact reconstructed canonical length, never the
    /// physical payload length. Queue and Sign transactions stay unchanged.
    pub fn send_compact_canonical(
        &mut self,
        port: PortId,
        logical_bytes: u32,
    ) -> Result<(), SchedulerError> {
        #[cfg(not(feature = "alloc"))]
        {
            let _ = (port, logical_bytes);
            return self.fail(SchedulerError::InvalidPlan);
        }
        #[cfg(feature = "alloc")]
        {
            self.consume_fuel(1)?;
            let index = usize::from(port.0);
            if self.compact_canonical_output != Some(port)
                || self
                    .output_maximum_bytes
                    .get(index)
                    .copied()
                    .flatten()
                    .is_none_or(|n| logical_bytes == 0 || n < logical_bytes)
                || self.outputs.get(index).is_none_or(Option::is_some)
                || self.canonical_output.is_some()
                || self.prepared_output.is_some()
            {
                return self.fail(SchedulerError::OutputBlocked);
            }
            self.prepared_output = Some((port, PreparedStepValue::CompactCanonical(logical_bytes)));
            Ok(())
        }
    }
}
#[cfg(feature = "alloc")]
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
    pub(super) fn check_compact_canonical_binding(&self) -> Result<(), SchedulerError> {
        if self
            .values
            .compact_canonical_layout()
            .is_some_and(|layout| {
                self.compact_canonical_binding
                    .as_ref()
                    .is_none_or(|binding| binding.layout != layout)
            })
        {
            return Err(SchedulerError::InvalidPlan);
        }
        Ok(())
    }
    pub(super) fn compact_canonical_input_binding(
        &self,
        node: usize,
        port: usize,
    ) -> Option<CompactCanonicalLayout> {
        self.compact_canonical_binding
            .as_ref()
            .filter(|b| usize::from(b.sink.0 .0) == node && usize::from(b.sink.1 .0) == port)
            .map(|b| b.layout)
    }
    pub(super) fn compact_canonical_output_binding(
        &self,
        node: usize,
    ) -> Option<(PortId, CompactCanonicalLayout)> {
        self.compact_canonical_binding
            .as_ref()
            .filter(|b| usize::from(b.source.0 .0) == node)
            .map(|b| (b.source.1, b.layout))
    }
    pub(super) fn has_compact_canonical_node(&self, node: usize) -> bool {
        self.compact_canonical_binding
            .as_ref()
            .is_some_and(|b| usize::from(b.source.0 .0) == node || usize::from(b.sink.0 .0) == node)
    }
    /// Bind one checked local edge without changing its Type, Plan or quota.
    /// Declarations alone cannot enter this trusted composition boundary.
    ///
    /// # Safety
    /// Caller must independently verify the exact checked Source/Plan, actual
    /// initialized producer and consumer, canonical Type and authored Forms.
    /// Their codec must reconstruct the exact original canonical bytes for all
    /// branches. The layout identity must bind those actual immutable facts.
    /// Matching strings, packet claims or StepBack declarations are insufficient.
    pub unsafe fn bind_compact_canonical_edge_from_trusted_make(
        &mut self,
        source: (NodeId, PortId),
        sink: (NodeId, PortId),
        layout: CompactCanonicalLayout,
    ) -> Result<(), SchedulerError> {
        if self.decisions != 0
            || self.cancelled
            || !self.debug_control.is_clear()
            || self.signs.requires_canonical_debug_values()
            || self.compact_canonical_binding.is_some()
            || source.0 == sink.0
            || usize::from(source.1 .0) >= P
            || usize::from(sink.1 .0) >= P
        {
            return Err(SchedulerError::InvalidPlan);
        }
        for node in [source.0, sink.0] {
            let n = usize::from(node.0);
            if n >= self.active_nodes
                || self.native_input_ports[n].is_some()
                || self.native_output_ports[n].is_some()
                || self.has_compact_f32_node(n)
                || self.region_successors[n].is_some()
                || self.region_successors.contains(&Some(node))
                || self.terminal_transductions[n].iter().any(Option::is_some)
                || self.host_bindings.as_ref().is_some_and(|t| {
                    t.bindings
                        .iter()
                        .enumerate()
                        .any(|(i, b)| b.is_some() && i / usize::from(t.maximum_gears_per_node) == n)
                })
            {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        if self.drivers[usize::from(source.0 .0)].compact_canonical_output()
            != Some((source.1, layout))
            || self.drivers[usize::from(sink.0 .0)].compact_canonical_input(sink.1) != Some(layout)
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut outgoing = self.cord_specs[..self.active_cords]
            .iter()
            .filter(|c| c.source_local() == Some(source));
        let cord = outgoing.next().ok_or(SchedulerError::InvalidPlan)?;
        if outgoing.next().is_some()
            || cord.sink_local() != Some(sink)
            || cord.track != AssignedConnectionTrack::Payload
            || cord.pressure_policy != AssignedPressurePolicy::PreserveOrder
            || cord.maximum_value_bytes != layout.logical_bytes()
            || cord.byte_capacity < layout.logical_bytes()
            || self.node_specs[usize::from(sink.0 .0)].input_cords[usize::from(sink.1 .0)]
                .and_then(|id| self.cord_specs.get(usize::from(id.0)))
                .is_none_or(|c| c.source_local() != Some(source))
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let required = u32::from(cord.item_capacity)
            .checked_add(1)
            .ok_or(SchedulerError::InvalidPlan)?;
        let (available, quota) = self.values.compact_canonical_available(layout);
        if u32::from(available) < required
            || required
                .checked_mul(layout.logical_bytes())
                .is_none_or(|n| quota < n)
        {
            return Err(SchedulerError::InvalidPlan);
        }
        // A finite descriptor lives inside the already admitted scheduler.
        // Binding cannot allocate after affine software owners transfer.
        self.compact_canonical_binding = Some(Binding {
            source,
            sink,
            layout,
        });
        Ok(())
    }
}
