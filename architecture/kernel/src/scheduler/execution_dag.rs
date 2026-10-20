//! Physical topological continuation, without creating logical connectivity.
use super::*;
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
    /// Bind the finite order of an already admitted pure local DAG. The Host
    /// proves semantic and resource eligibility; no Host Call, terminal law or
    /// existing region may be absorbed here. An escaping output ends physical
    /// continuation so its boundary survives. Readiness, per-Gear fuel, commit,
    /// cancellation checkpoints and mandatory Signs stay in the same kernel.
    pub fn bind_execution_dag(&mut self, nodes: &[NodeId]) -> Result<(), SchedulerError> {
        if !(2..=32).contains(&nodes.len())
            || self.decisions != 0
            || self.cancelled
            || !self.debug_control.is_clear()
        {
            return Err(SchedulerError::InvalidPlan);
        }
        for (i, node) in nodes.iter().enumerate() {
            let n = usize::from(node.0);
            if n >= self.active_nodes
                || nodes[..i].contains(node)
                || self.region_successors[n].is_some()
                || self.region_successors.contains(&Some(*node))
                || self.native_input_ports[n].is_some()
                || self.native_output_ports[n].is_some()
                || self.has_compact_f32_node(n)
                || self.terminal_transductions[n].iter().any(Option::is_some)
                || self.host_bindings.as_ref().is_some_and(|table| {
                    table.bindings.iter().enumerate().any(|(slot, b)| {
                        b.is_some() && slot / usize::from(table.maximum_gears_per_node) == n
                    })
                })
            {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        let mut reached = [false; 32];
        reached[0] = true;
        for _ in 0..nodes.len() {
            for cord in &self.cord_specs[..self.active_cords] {
                let (Some((source, _)), Some((sink, _))) = (cord.source_local(), cord.sink_local())
                else {
                    continue;
                };
                let (Some(a), Some(b)) = (
                    nodes.iter().position(|n| *n == source),
                    nodes.iter().position(|n| *n == sink),
                ) else {
                    continue;
                };
                if a >= b
                    || cord.track != AssignedConnectionTrack::Payload
                    || cord.pressure_policy != AssignedPressurePolicy::PreserveOrder
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                if reached[a] || reached[b] {
                    reached[a] = true;
                    reached[b] = true;
                }
            }
        }
        if reached[..nodes.len()].iter().any(|b| !*b) {
            return Err(SchedulerError::InvalidPlan);
        }
        // These are dispatch successors only. The original routes are untouched.
        // A self marker retains membership even at an escaping boundary. It is
        // never a logical edge or an executable physical continuation.
        for node in nodes {
            self.region_successors[usize::from(node.0)] = Some(*node);
        }
        for pair in nodes.windows(2) {
            let mut outgoing = self.cord_specs[..self.active_cords]
                .iter()
                .filter(|c| c.source_local().is_some_and(|(n, _)| n == pair[0]))
                .peekable();
            if outgoing.peek().is_some()
                && outgoing.all(|c| c.sink_local().is_some_and(|(n, _)| nodes.contains(&n)))
            {
                self.region_successors[usize::from(pair[0].0)] = Some(pair[1]);
            }
        }
        Ok(())
    }
}
