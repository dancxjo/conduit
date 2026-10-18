//! Bounded physical dispatch over unchanged logical Step transactions.
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
    /// Bind an already admitted pure chain before Play. Logical tables and
    /// transport remain authoritative. This API neither proves semantic purity
    /// nor selects a Back: those are the exact Plan/Host preparation obligations.
    /// The reviewed installed region profile requires `alloc` preparation;
    /// no-alloc products refuse binding and retain their single-Step path.
    pub fn bind_execution_chain(&mut self, nodes: &[NodeId]) -> Result<(), SchedulerError> {
        if !cfg!(feature = "alloc")
            || self.cancelled
            || self.decisions != 0
            || !(2..=32).contains(&nodes.len())
        {
            return Err(SchedulerError::InvalidPlan);
        }
        for (index, node) in nodes.iter().enumerate() {
            if usize::from(node.0) >= self.active_nodes
                || nodes[..index].contains(node)
                || self.region_successors[usize::from(node.0)].is_some()
                || self.region_successors.contains(&Some(*node))
                || self.host_bindings.as_ref().is_some_and(|table| {
                    table.bindings.iter().enumerate().any(|(slot, binding)| {
                        binding.is_some()
                            && slot / usize::from(table.maximum_gears_per_node)
                                == usize::from(node.0)
                    })
                })
            {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        for pair in nodes.windows(2) {
            let links = self.cord_specs[..self.active_cords]
                .iter()
                .filter(|cord| {
                    matches!(cord.source, CordEndpoint::Local { node, .. } if node == pair[0])
                        && matches!(cord.sink, CordEndpoint::Local { node, .. } if node == pair[1])
                        && cord.track == AssignedConnectionTrack::Payload
                })
                .count();
            let outgoing = self.cord_specs[..self.active_cords].iter().filter(|cord|
                matches!(cord.source, CordEndpoint::Local { node, .. } if node == pair[0])).count();
            let incoming = self.cord_specs[..self.active_cords]
                .iter()
                .filter(
                    |cord| matches!(cord.sink, CordEndpoint::Local { node, .. } if node == pair[1]),
                )
                .count();
            if links != 1 || outgoing != 1 || incoming != 1 {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        for pair in nodes.windows(2) {
            self.region_successors[usize::from(pair[0].0)] = Some(pair[1]);
        }
        Ok(())
    }
    pub(super) fn step_region(
        &mut self,
        proceed: &mut impl FnMut(NodeId) -> bool,
    ) -> Result<SchedulerStatus, SchedulerError> {
        if self.cancelled {
            return Ok(SchedulerStatus::Cancelled);
        }
        if self.debug_control.suspension().is_some() {
            return Err(SchedulerError::DebugSuspended);
        }
        let Some(mut node) = self.next_ready() else {
            if let Some(node) = self.emit_quiescence_transitions()? {
                return Ok(SchedulerStatus::Progress { node });
            }
            return if self.all_nodes_completed()
                && self.cords[..self.active_cords]
                    .iter()
                    .all(|cord| cord.len == 0)
            {
                self.drained_status()
            } else {
                Ok(SchedulerStatus::Idle)
            };
        };
        let mut last_executed = None;
        // Each selected region advances at most its admitted finite chain.
        // The execution owner can request cancellation at every logical checkpoint.
        for ordinal in 0..32 {
            let node_id = NodeId(as_u16(node)?);
            if !proceed(node_id) {
                return Ok(last_executed.map_or(SchedulerStatus::Idle, |node| {
                    SchedulerStatus::Progress { node }
                }));
            }
            if ordinal == 0 {
                if self.debug_control.suspend_before(node_id) {
                    return Err(SchedulerError::DebugSuspended);
                }
                self.decisions = self
                    .decisions
                    .checked_add(1)
                    .ok_or(SchedulerError::DecisionLimitExceeded)?;
            }
            let may_continue = self.step_logical(node)?;
            last_executed = Some(node_id);
            // No reviewed no-alloc Host offers this physical profile. Keep its
            // fixed private images on the original single logical Step path.
            if !cfg!(feature = "alloc") {
                break;
            }
            let Some(next) = self.region_successors[node] else {
                break;
            };
            let next = usize::from(next.0);
            if !may_continue || !self.ready.contains(next) || self.completed[next] {
                break;
            }
            node = next;
            self.cursor = (node + 1) % self.active_nodes;
        }
        if self.all_nodes_completed()
            && self.cords[..self.active_cords]
                .iter()
                .all(|cord| cord.len == 0)
        {
            self.drained_status()
        } else {
            Ok(SchedulerStatus::Progress {
                node: NodeId(as_u16(node)?),
            })
        }
    }

    fn all_nodes_completed(&self) -> bool {
        if let Some(count) = self.ready.completion_count() {
            count == self.active_nodes
        } else {
            // A native word fits once capacity is at least twice its width.
            // Below that threshold the original early-exit scan is bounded by
            // fifteen nodes on supported targets, preserving tiny-Host work.
            self.completed[..self.active_nodes].iter().all(|done| {
                #[cfg(test)]
                {
                    let mut counts = self.scan_counts.get();
                    counts.terminal_node_probes += 1;
                    self.scan_counts.set(counts);
                }
                *done
            })
        }
    }

    /// Both terminal outcomes monotonically retire the same active membership.
    /// Keep the flag and its acceleration count together, including duplicate
    /// internal notifications. Cancellation remains a separate terminal state.
    pub(super) fn mark_completed(&mut self, node: usize) {
        debug_assert!(node < self.active_nodes);
        if !self.completed[node] {
            self.completed[node] = true;
            self.ready.note_completion();
        }
    }

    #[cfg(test)]
    fn completed_count(&self) -> usize {
        self.ready.completion_count().unwrap_or_else(|| {
            self.completed[..self.active_nodes]
                .iter()
                .filter(|done| **done)
                .count()
        })
    }

    fn step_logical(&mut self, node: usize) -> Result<bool, SchedulerError> {
        if self.debug_control.suspend_before(NodeId(as_u16(node)?)) {
            return Err(SchedulerError::DebugSuspended);
        }
        self.signs.record(
            NodeId(as_u16(node)?),
            None,
            None,
            KernelEventKind::StepFuelGranted,
        )?;
        self.signs.observe_debug(DebugRuntimeEvent {
            node: NodeId(as_u16(node)?),
            port: None,
            cord: None,
            kind: DebugEventKind::GearStarted,
            type_identity: None,
            value: None,
            fault_code: None,
        });

        let mut io = self.context(node)?;
        let mut current_input_bytes = [None; PORTS];
        for (port, value) in io.inputs.iter().copied().enumerate() {
            if let Some(value) = value {
                current_input_bytes[port] = Some(self.values.get(value)?);
            }
        }
        let input_bytes = StepInputBytes {
            inputs: current_input_bytes,
            host_output: io
                .host_completion
                .and_then(|(_, outcome)| outcome.output)
                .map(|output| self.values.get(output.value))
                .transpose()?,
        };
        self.logical_invocations = self
            .logical_invocations
            .checked_add(1)
            .ok_or(SchedulerError::DecisionLimitExceeded)?;
        let outcome = self.drivers[node].step(&mut io, &input_bytes);
        if let Some(fault) = io.fault {
            if fault == SchedulerError::StepFuelExceeded {
                self.signs.record(
                    NodeId(as_u16(node)?),
                    None,
                    None,
                    KernelEventKind::StepFuelExceeded,
                )?;
            }
            return Err(fault);
        }
        let may_continue = matches!(
            outcome,
            StepOutcome::Progress | StepOutcome::Complete | StepOutcome::Await
        );
        self.apply_step(node, outcome, io)?;
        Ok(may_continue)
    }
}
