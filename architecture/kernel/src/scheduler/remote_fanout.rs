//! Exact atomic remote ingress payload and terminal fan-out.
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
    /// Atomically admits one external payload to every exact ingress branch.
    /// If any branch is pressured, no branch observes the value and the same
    /// sequence remains retryable across the whole fan-out.
    pub fn admit_remote_input_fanout(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        if targets.is_empty() || targets.len() > usize::from(u16::MAX) {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let byte_len =
            u32::try_from(bytes.len()).map_err(|_| SchedulerError::QueueByteCapacityExceeded)?;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(SchedulerError::RemoteSequenceRejected)?;
        for (index, (endpoint, cord)) in targets.iter().copied().enumerate() {
            if targets[..index].contains(&(endpoint, cord)) {
                return Err(SchedulerError::InvalidRemoteCordAccess);
            }
            let cord_index = usize::from(cord.0);
            if cord_index >= self.active_cords {
                return Err(SchedulerError::InvalidRemoteCordAccess);
            }
            let spec = self.cord_specs[cord_index];
            if !matches!(spec.source, CordEndpoint::Remote(candidate) if candidate == endpoint)
                || !matches!(spec.sink, CordEndpoint::Local { .. })
            {
                return Err(SchedulerError::InvalidRemoteCordAccess);
            }
            let state = &self.cords[cord_index];
            if state.producer_closed || state.next_remote_sequence != sequence {
                return Err(SchedulerError::RemoteSequenceRejected);
            }
            if byte_len > self.admission_maximum(spec, state)? {
                return Ok(RemoteIngressOutcome::Full { sequence });
            }
        }
        self.ensure_sign_capacity(targets.len())?;
        self.ensure_remote_sign_capacity(targets.len())?;
        let value = self.values.store(bytes)?;
        for references in 1..targets.len() {
            if let Err(error) = self.values.retain(value) {
                for _ in 0..references {
                    self.values.release(value)?;
                }
                return Err(error.into());
            }
        }
        for (endpoint, cord) in targets.iter().copied() {
            let cord_index = usize::from(cord.0);
            let spec = self.cord_specs[cord_index];
            let (sink_node, sink_port) = spec.sink_local().ok_or(SchedulerError::InvalidPlan)?;
            if let Some(superseded) = self.push(cord_index, value)? {
                self.values.release(superseded)?;
            }
            self.cords[cord_index].next_remote_sequence = next_sequence;
            self.ready[usize::from(sink_node.0)] = true;
            self.signs.record_remote(
                sink_node,
                sink_port,
                KernelEventKind::RemoteInputAdmitted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord,
                    direction: crate::RemoteCordDirection::Ingress,
                    sequence,
                },
            )?;
        }
        Ok(RemoteIngressOutcome::Accepted { sequence })
    }

    /// Validate all targets and reserve all mandatory Sign storage before
    /// closing any branch. An invalid target or pressure leaves every branch unchanged.
    pub fn close_remote_input_fanout(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
    ) -> Result<(), SchedulerError> {
        self.close_remote_input_fanout_terminal(targets, None)
    }

    pub fn close_remote_input_fanout_abnormal(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        terminal: CanonicalValue,
    ) -> Result<(), SchedulerError> {
        self.close_remote_input_fanout_terminal(targets, Some(terminal))
    }

    fn close_remote_input_fanout_terminal(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        terminal: Option<CanonicalValue>,
    ) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        if targets.is_empty() || targets.len() > self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let mut open = 0;
        for (index, (endpoint, cord)) in targets.iter().copied().enumerate() {
            if targets[..index].contains(&(endpoint, cord)) {
                return Err(SchedulerError::InvalidRemoteCordAccess);
            }
            let index = usize::from(cord.0);
            if index >= self.active_cords {
                return Err(SchedulerError::InvalidRemoteCordAccess);
            }
            let spec = self.cord_specs[index];
            if !matches!(spec.source, CordEndpoint::Remote(candidate) if candidate == endpoint)
                || !matches!(spec.sink, CordEndpoint::Local { .. })
            {
                return Err(SchedulerError::InvalidRemoteCordAccess);
            }
            let state = &self.cords[index];
            if state.producer_closed {
                if state.abnormal_terminal != terminal {
                    return Err(SchedulerError::RemoteDeliveryRejected);
                }
            } else {
                open += 1;
            }
        }
        self.ensure_sign_capacity(open)?;
        self.ensure_remote_sign_capacity(open)?;
        for (endpoint, cord) in targets.iter().copied() {
            self.close_remote_input_terminal(endpoint, cord, terminal)?;
        }
        Ok(())
    }
}
