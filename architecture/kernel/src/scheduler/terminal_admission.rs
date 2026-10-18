//! Terminal byte and publication evidence admission before Step commit.
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
    /// Reserve remote lifecycle storage and count every ordinary terminal Sign
    /// and check each terminal value fits its cord before committing inputs,
    /// Back state, or the node's completed flag.
    pub(super) fn terminal_publication_signs(
        &self,
        node: usize,
        outcome: StepOutcome,
    ) -> Result<usize, SchedulerError> {
        let (track, output, terminal_bytes) = match outcome {
            StepOutcome::Complete => (AssignedConnectionTrack::NormalClose, None, 0),
            StepOutcome::Abnormal { port, terminal } => (
                AssignedConnectionTrack::AbnormalTerminal,
                Some(port),
                terminal.as_slice().len() as u32,
            ),
            _ => return Ok(0),
        };
        let source = NodeId(as_u16(node)?);
        let mut values = 0_usize;
        let mut remote = 0_usize;
        for port in 0..PORTS {
            let port = PortId(as_u16(port)?);
            if output.is_some_and(|output| output != port) {
                continue;
            }
            let Ok(targets) = self.routes.route(source, port) else {
                continue;
            };
            for target in targets {
                let spec = self.cord_specs[usize::from(target.cord.0)];
                if spec.track == track {
                    // This refusal cannot be repaired by consuming a queued
                    // input during commit: the value alone exceeds the budget.
                    if spec.byte_capacity < terminal_bytes {
                        return Err(SchedulerError::QueueCapacityExceeded);
                    }
                    if terminal_bytes > spec.maximum_value_bytes {
                        return Err(SchedulerError::SemanticValueBoundExceeded);
                    }
                    values = values.checked_add(1).ok_or(SchedulerError::InvalidPlan)?;
                }
                if matches!(target.sink, CordEndpoint::Remote(_)) {
                    remote = remote.checked_add(1).ok_or(SchedulerError::InvalidPlan)?;
                }
            }
        }
        // An unrouted abnormal terminal ends at this semantic boundary; the
        // publisher does not close sibling tracks without an abnormal target.
        if output.is_some() && values == 0 {
            return Ok(0);
        }
        self.ensure_remote_sign_capacity(remote)?;
        values
            .checked_add(remote)
            .ok_or(SchedulerError::InvalidPlan)
    }
}
