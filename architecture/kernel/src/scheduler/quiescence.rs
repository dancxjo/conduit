//! Quiescence arming and wakeable-edge emission for the fixed scheduler.
use super::*;

/// Prepared once from the validated active prefix. Cord specifications are
/// immutable for the scheduler lifetime; padding cannot enable runtime work.
pub(super) fn has_quiescence_cords(active_cords: &[CordSpec]) -> bool {
    active_cords
        .iter()
        .any(|cord| cord.track == AssignedConnectionTrack::Quiescence)
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
    pub(super) fn arm_quiescence_outputs(&mut self, node: usize) -> Result<(), SchedulerError> {
        let node = NodeId(as_u16(node)?);
        if !self.has_quiescence_cords {
            return Ok(());
        }
        // Plan validation proves every local-source cord is in this source's
        // exact route and that routed targets are active and unique per port.
        for port in 0..PORTS {
            let Ok(targets) = self.routes.route(node, PortId(as_u16(port)?)) else {
                continue;
            };
            for target in targets {
                let cord = usize::from(target.cord.0);
                #[cfg(test)]
                {
                    let mut counts = self.scan_counts.get();
                    counts.cord_scans += 1;
                    self.scan_counts.set(counts);
                }
                if self.cord_specs[cord].track == AssignedConnectionTrack::Quiescence {
                    self.cords[cord].quiescence_armed = true;
                }
            }
        }
        Ok(())
    }

    /// Emit one wakeable edge for every source that became active and has now
    /// returned the whole play to quiescence. Empty queues alone do not arm an
    /// edge, and terminal sources are never reported as quiescent.
    pub(super) fn emit_quiescence_transitions(&mut self) -> Result<Option<NodeId>, SchedulerError> {
        if !self.has_quiescence_cords {
            return Ok(None);
        }
        let mut count = 0_usize;
        let mut first_source = None;
        for cord in 0..self.active_cords {
            #[cfg(test)]
            {
                let mut counts = self.scan_counts.get();
                counts.cord_scans += 1;
                self.scan_counts.set(counts);
            }
            let spec = self.cord_specs[cord];
            if spec.track != AssignedConnectionTrack::Quiescence
                || !self.cords[cord].quiescence_armed
            {
                continue;
            }
            let Some((source, _)) = spec.source_local() else {
                continue;
            };
            if self.completed[usize::from(source.0)] || self.cords[cord].len != 0 {
                continue;
            }
            first_source.get_or_insert(source);
            count = count.checked_add(1).ok_or(SchedulerError::InvalidPlan)?;
        }
        if count == 0 {
            return Ok(None);
        }
        self.ensure_sign_capacity(count)?;
        let value = self.values.store(&[])?;
        for _ in 1..count {
            self.values.retain(value)?;
        }
        for cord in 0..self.active_cords {
            #[cfg(test)]
            {
                let mut counts = self.scan_counts.get();
                counts.cord_scans += 1;
                self.scan_counts.set(counts);
            }
            let spec = self.cord_specs[cord];
            if spec.track != AssignedConnectionTrack::Quiescence
                || !self.cords[cord].quiescence_armed
            {
                continue;
            }
            let Some((source, port)) = spec.source_local() else {
                continue;
            };
            if self.completed[usize::from(source.0)] || self.cords[cord].len != 0 {
                continue;
            }
            self.push(cord, value)?;
            self.cords[cord].quiescence_armed = false;
            if let Some((sink, _)) = spec.sink_local() {
                self.ready.set(usize::from(sink.0), true);
            }
            self.signs
                .record(source, Some(port), None, KernelEventKind::ValueRouted)?;
        }
        Ok(first_source)
    }
}

#[cfg(test)]
mod tests;
