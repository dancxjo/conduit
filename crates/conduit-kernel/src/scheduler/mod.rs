//! Fixed-capacity deterministic scheduler over the port-aware kernel contract.

use crate::{
    BoundedValueRef, CordEndpoint, CordId, EvidenceError, EvidenceSink, FixedHostOperationBindings,
    FixedRoutes, HostOperationBinding, HostOperationId, HostOperationOutcome, KernelEventKind,
    NodeId, OperationAction, PortId, RemoteEndpointId, RequestId, RouteTarget, StorageError,
    ValueRef, ValueStorage,
};

pub mod operation;
pub mod specs;

pub use operation::{
    HostOperationRequest, OperationDriver, SchedulerError, SchedulerStatus, StepIo,
    StepOperation, StepOutcome,
};
pub use specs::{CordCapacity, CordSpec, NodeSpec, RemoteIngressOutcome, RemoteValueOffer};

use operation::{PendingHostOperation, StagedStep};

#[derive(Clone, Copy, Debug)]
struct CordState {
    head: u16,
    len: u16,
    queued_bytes: u32,
    producer_closed: bool,
    next_remote_sequence: u64,
    offered_remote_sequence: Option<u64>,
    remote_accepted: bool,
}

impl CordState {
    const EMPTY: Self = Self {
        head: 0,
        len: 0,
        queued_bytes: 0,
        producer_closed: false,
        next_remote_sequence: 0,
        offered_remote_sequence: None,
        remote_accepted: false,
    };
}

pub struct FixedScheduler<
    D,
    S,
    E,
    const NODES: usize,
    const CORDS: usize,
    const PORTS: usize,
    const QUEUE_SLOTS: usize,
    const ROUTE_SLOTS: usize,
    const ROUTE_TARGETS: usize,
    const HOST_BINDING_SLOTS: usize = 0,
    const PENDING_REQUESTS: usize = 0,
> where
    D: StepOperation<PORTS>,
    S: ValueStorage,
    E: EvidenceSink,
{
    node_specs: [NodeSpec<PORTS>; NODES],
    cord_specs: [CordSpec; CORDS],
    routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
    host_bindings: Option<FixedHostOperationBindings<HOST_BINDING_SLOTS>>,
    pending_host_operations: [Option<PendingHostOperation>; PENDING_REQUESTS],
    drivers: [D; NODES],
    values: S,
    evidence: E,
    cords: [CordState; CORDS],
    queue_slots: [Option<ValueRef>; QUEUE_SLOTS],
    ready: [bool; NODES],
    completed: [bool; NODES],
    cursor: usize,
    decisions: u32,
    last_host_request: [Option<RequestId>; NODES],
    cancelled: bool,
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
    D: StepOperation<PORTS>,
    S: ValueStorage,
    E: EvidenceSink,
{
    pub fn new(
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        drivers: [D; NODES],
        values: S,
        evidence: E,
    ) -> Result<Self, SchedulerError> {
        if NODES == 0 || CORDS == 0 || PORTS == 0 || QUEUE_SLOTS == 0 || !routes.is_sealed() {
            return Err(SchedulerError::InvalidPlan);
        }
        validate_plan::<NODES, CORDS, PORTS, QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS>(
            &node_specs,
            &cord_specs,
            &routes,
        )?;
        Ok(Self {
            node_specs,
            cord_specs,
            routes,
            host_bindings: None,
            pending_host_operations: [None; PENDING_REQUESTS],
            drivers,
            values,
            evidence,
            cords: [CordState::EMPTY; CORDS],
            queue_slots: [None; QUEUE_SLOTS],
            ready: [true; NODES],
            completed: [false; NODES],
            cursor: 0,
            decisions: 0,
            last_host_request: [None; NODES],
            cancelled: false,
        })
    }

    pub fn new_with_host_operations(
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        host_bindings: FixedHostOperationBindings<HOST_BINDING_SLOTS>,
        drivers: [D; NODES],
        values: S,
        evidence: E,
    ) -> Result<Self, SchedulerError> {
        if PENDING_REQUESTS == 0 || !host_bindings.is_sealed() {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut scheduler = Self::new(node_specs, cord_specs, routes, drivers, values, evidence)?;
        scheduler.host_bindings = Some(host_bindings);
        Ok(scheduler)
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        if self.cancelled {
            return Ok(SchedulerStatus::Cancelled);
        }
        let Some(node) = self.next_ready() else {
            return if self.completed.iter().all(|value| *value)
                && self.cords.iter().all(|cord| cord.len == 0)
            {
                Ok(SchedulerStatus::Complete)
            } else {
                Ok(SchedulerStatus::Idle)
            };
        };
        self.decisions = self
            .decisions
            .checked_add(1)
            .ok_or(SchedulerError::DecisionLimitExceeded)?;
        self.evidence
            .record(NodeId(as_u16(node)?), None, None, KernelEventKind::Decision)?;

        let mut io = self.context(node)?;
        let outcome = self.drivers[node].step(&mut io);
        if let Some(fault) = io.fault {
            return Err(fault);
        }
        self.apply_step(node, outcome, io)?;
        if self.completed.iter().all(|value| *value) && self.cords.iter().all(|cord| cord.len == 0)
        {
            Ok(SchedulerStatus::Complete)
        } else {
            Ok(SchedulerStatus::Progress {
                node: NodeId(as_u16(node)?),
            })
        }
    }

    pub fn run(&mut self, maximum_decisions: u32) -> Result<(), SchedulerError> {
        if maximum_decisions == 0 {
            return Err(SchedulerError::DecisionLimitExceeded);
        }
        for _ in 0..maximum_decisions {
            match self.step()? {
                SchedulerStatus::Complete => return Ok(()),
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => return Err(SchedulerError::FalseProgress),
                SchedulerStatus::Cancelled => return Err(SchedulerError::Cancelled),
            }
        }
        Err(SchedulerError::DecisionLimitExceeded)
    }

    pub fn decisions(&self) -> u32 {
        self.decisions
    }

    pub fn drivers(&self) -> &[D; NODES] {
        &self.drivers
    }

    pub fn values(&self) -> &S {
        &self.values
    }

    pub fn evidence(&self) -> &E {
        &self.evidence
    }

    pub fn cord_usage(&self, cord: CordId) -> Result<(u16, u32), SchedulerError> {
        let state = self
            .cords
            .get(usize::from(cord.0))
            .ok_or(SchedulerError::InvalidPlan)?;
        Ok((state.len, state.queued_bytes))
    }

    /// Offers the head of a remote egress cord without transferring ownership.
    /// Repeated calls return the same sequence and value until exact delivery
    /// is acknowledged.
    pub fn remote_egress_offer(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<RemoteValueOffer>, SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        let cord_index = usize::from(cord.0);
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (source_node, source_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Local { node, port }, CordEndpoint::Remote(candidate))
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess),
        };
        let Some(value) = self.peek(cord_index)? else {
            return Ok(None);
        };
        let existing = self.cords[cord_index].offered_remote_sequence;
        let sequence = existing.unwrap_or(self.cords[cord_index].next_remote_sequence);
        if existing.is_none() {
            self.ensure_evidence_capacity(1)?;
            self.cords[cord_index].offered_remote_sequence = Some(sequence);
            self.evidence.record(
                source_node,
                Some(source_port),
                None,
                KernelEventKind::RemoteValueOffered,
            )?;
        }
        Ok(Some(RemoteValueOffer {
            endpoint,
            cord,
            sequence,
            value,
        }))
    }

    pub fn remote_egress_accept(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
    ) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        let cord_index = usize::from(cord.0);
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (source_node, source_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Local { node, port }, CordEndpoint::Remote(candidate))
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess),
        };
        let state = self
            .cords
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if state.offered_remote_sequence != Some(sequence) {
            return Err(SchedulerError::RemoteSequenceRejected);
        }
        if state.remote_accepted {
            return Ok(());
        }
        self.ensure_evidence_capacity(1)?;
        self.cords[cord_index].remote_accepted = true;
        self.evidence.record(
            source_node,
            Some(source_port),
            None,
            KernelEventKind::RemoteValueAccepted,
        )?;
        Ok(())
    }

    /// Releases the source value only after the carrier reports the exact
    /// sequence as delivered by the peer kernel.
    pub fn remote_egress_delivered(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
    ) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::RemoteDeliveryRejected);
        }
        let cord_index = usize::from(cord.0);
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (source_node, source_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Local { node, port }, CordEndpoint::Remote(candidate))
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess),
        };
        let state = self
            .cords
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if state.offered_remote_sequence != Some(sequence) || !state.remote_accepted {
            return Err(SchedulerError::RemoteDeliveryRejected);
        }
        let next_sequence = state
            .next_remote_sequence
            .checked_add(1)
            .ok_or(SchedulerError::RemoteSequenceRejected)?;
        self.ensure_evidence_capacity(1)?;
        let value = self.pop(cord_index)?;
        self.values.release(value)?;
        let state = &mut self.cords[cord_index];
        state.next_remote_sequence = next_sequence;
        state.offered_remote_sequence = None;
        state.remote_accepted = false;
        self.ready[usize::from(source_node.0)] = true;
        self.evidence.record(
            source_node,
            Some(source_port),
            None,
            KernelEventKind::RemoteValueDelivered,
        )?;
        Ok(())
    }

    /// Admits bytes through a remote ingress cord into the kernel-owned value
    /// store and queue. `Full` performs no allocation or sequence advance, so
    /// the carrier must retry the same sequence.
    pub fn admit_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        let cord_index = usize::from(cord.0);
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (sink_node, sink_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Remote(candidate), CordEndpoint::Local { node, port })
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess),
        };
        let state = self
            .cords
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if state.producer_closed || state.next_remote_sequence != sequence {
            return Err(SchedulerError::RemoteSequenceRejected);
        }
        let byte_len =
            u32::try_from(bytes.len()).map_err(|_| SchedulerError::QueueByteCapacityExceeded)?;
        if state.len >= spec.item_capacity
            || byte_len > spec.byte_capacity.saturating_sub(state.queued_bytes)
        {
            return Ok(RemoteIngressOutcome::Full { sequence });
        }
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(SchedulerError::RemoteSequenceRejected)?;
        self.ensure_evidence_capacity(1)?;
        let value = self.values.store(bytes)?;
        if let Err(error) = self.push(cord_index, value) {
            self.values.release(value)?;
            return Err(error);
        }
        self.cords[cord_index].next_remote_sequence = next_sequence;
        self.ready[usize::from(sink_node.0)] = true;
        self.evidence.record(
            sink_node,
            Some(sink_port),
            None,
            KernelEventKind::RemoteInputAdmitted,
        )?;
        Ok(RemoteIngressOutcome::Accepted { sequence })
    }

    pub fn close_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        let cord_index = usize::from(cord.0);
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (sink_node, sink_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Remote(candidate), CordEndpoint::Local { node, port })
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess),
        };
        if self.cords[cord_index].producer_closed {
            return Ok(());
        }
        self.ensure_evidence_capacity(1)?;
        self.cords[cord_index].producer_closed = true;
        self.ready[usize::from(sink_node.0)] = true;
        self.evidence.record(
            sink_node,
            Some(sink_port),
            None,
            KernelEventKind::RemoteInputClosed,
        )?;
        Ok(())
    }

    pub fn remote_egress_terminal(
        &self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<bool, SchedulerError> {
        let cord_index = usize::from(cord.0);
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if !matches!(
            (spec.source, spec.sink),
            (
                CordEndpoint::Local { .. },
                CordEndpoint::Remote(candidate)
            ) if candidate == endpoint
        ) {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let state = self.cords[cord_index];
        Ok(state.producer_closed && state.len == 0)
    }

    pub fn next_host_request(&mut self) -> Option<HostOperationRequest> {
        let pending = self
            .pending_host_operations
            .iter_mut()
            .flatten()
            .find(|pending| !pending.dispatched)?;
        pending.dispatched = true;
        Some(pending.request)
    }

    pub fn complete_host_operation(
        &mut self,
        node: NodeId,
        request: RequestId,
        outcome: HostOperationOutcome,
    ) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::HostOperationCompletionRejected);
        }
        let slot = self
            .pending_host_operations
            .iter()
            .position(|pending| {
                pending
                    .map(|pending| {
                        pending.request.node == node && pending.request.request == request
                    })
                    .unwrap_or(false)
            })
            .ok_or(SchedulerError::HostOperationCompletionRejected)?;
        let pending = self.pending_host_operations[slot]
            .ok_or(SchedulerError::HostOperationCompletionRejected)?;
        if !pending.dispatched || pending.completion.is_some() {
            return Err(SchedulerError::HostOperationCompletionRejected);
        }
        if let Some(output) = outcome.output {
            if output.admitted_bytes == 0
                || output.value.byte_len > output.admitted_bytes
                || output.admitted_bytes > pending.maximum_output_bytes
                || output.value.byte_len > pending.maximum_output_bytes
            {
                return Err(SchedulerError::HostOperationOutputExceeded);
            }
            self.values.get(output.value)?;
        }
        self.ensure_evidence_capacity(1)?;
        if outcome.output.map(|output| output.value) != Some(pending.request.input.value) {
            self.values.release(pending.request.input.value)?;
        }
        self.pending_host_operations[slot]
            .as_mut()
            .ok_or(SchedulerError::HostOperationCompletionRejected)?
            .completion = Some(outcome);
        self.ready[usize::from(pending.request.node.0)] = true;
        self.evidence.record(
            pending.request.node,
            None,
            Some(request),
            KernelEventKind::HostOperationCompleted,
        )?;
        Ok(())
    }

    pub fn store_host_value(&mut self, bytes: &[u8]) -> Result<ValueRef, SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        Ok(self.values.store(bytes)?)
    }

    pub fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        Ok(self.values.get(value)?)
    }

    pub fn discard_host_value(&mut self, value: ValueRef) -> Result<(), SchedulerError> {
        if self
            .pending_host_operations
            .iter()
            .flatten()
            .any(|pending| {
                pending.request.input.value == value
                    || pending
                        .completion
                        .and_then(|outcome| outcome.output)
                        .map(|output| output.value)
                        == Some(value)
            })
            || self
                .queue_slots
                .iter()
                .flatten()
                .any(|queued| *queued == value)
        {
            return Err(SchedulerError::ValueOwnershipViolation);
        }
        Ok(self.values.release(value)?)
    }

    pub fn pending_host_operation_count(&self) -> usize {
        self.pending_host_operations
            .iter()
            .filter(|pending| pending.is_some())
            .count()
    }

    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Ok(());
        }
        self.ensure_evidence_capacity(2)?;
        self.evidence.record(
            NodeId(0),
            None,
            None,
            KernelEventKind::CancellationRequested,
        )?;
        for (node, driver) in self.drivers.iter_mut().enumerate() {
            if !self.completed[node] {
                driver.cancel();
            }
        }
        self.values.clear();
        self.pending_host_operations.fill(None);
        self.queue_slots.fill(None);
        for cord in &mut self.cords {
            cord.head = 0;
            cord.len = 0;
            cord.queued_bytes = 0;
            cord.producer_closed = true;
            cord.offered_remote_sequence = None;
            cord.remote_accepted = false;
        }
        self.ready.fill(false);
        self.cancelled = true;
        self.evidence
            .record(NodeId(0), None, None, KernelEventKind::RunCancelled)?;
        Ok(())
    }

    fn next_ready(&mut self) -> Option<usize> {
        for offset in 0..NODES {
            let node = (self.cursor + offset) % NODES;
            let waiting_for_host_completion =
                self.pending_host_operations
                    .iter()
                    .flatten()
                    .any(|pending| {
                        usize::from(pending.request.node.0) == node && pending.completion.is_none()
                    });
            if self.ready[node] && !self.completed[node] && !waiting_for_host_completion {
                self.cursor = (node + 1) % NODES;
                return Some(node);
            }
        }
        None
    }

    fn context(&self, node: usize) -> Result<StepIo<PORTS>, SchedulerError> {
        let mut inputs = [None; PORTS];
        let mut input_closed = [false; PORTS];
        let mut output_maximum_bytes = [None; PORTS];
        let host_completion = self
            .pending_host_operations
            .iter()
            .flatten()
            .find(|pending| usize::from(pending.request.node.0) == node)
            .and_then(|pending| {
                pending
                    .completion
                    .map(|outcome| (pending.request.request, outcome))
            });
        for (port, cord) in self.node_specs[node]
            .input_cords
            .iter()
            .copied()
            .enumerate()
        {
            let Some(cord) = cord else {
                continue;
            };
            let cord_index = usize::from(cord.0);
            inputs[port] = self.peek(cord_index)?;
            input_closed[port] =
                self.cords[cord_index].producer_closed && self.cords[cord_index].len == 0;
        }
        for (port, output_maximum) in output_maximum_bytes.iter_mut().enumerate() {
            let Ok(targets) = self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))
            else {
                continue;
            };
            let mut maximum = u32::MAX;
            let mut any = false;
            for target in targets {
                let cord = usize::from(target.cord.0);
                let state = self.cords.get(cord).ok_or(SchedulerError::InvalidPlan)?;
                let spec = self
                    .cord_specs
                    .get(cord)
                    .ok_or(SchedulerError::InvalidPlan)?;
                if state.producer_closed || state.len >= spec.item_capacity {
                    maximum = 0;
                    any = true;
                    break;
                }
                maximum = maximum.min(spec.byte_capacity.saturating_sub(state.queued_bytes));
                any = true;
            }
            if any && maximum > 0 {
                *output_maximum = Some(maximum);
            }
        }
        Ok(StepIo {
            inputs,
            input_closed,
            output_maximum_bytes,
            consumed: [false; PORTS],
            retained_inputs: [false; PORTS],
            consumed_closed: [false; PORTS],
            outputs: [None; PORTS],
            discards: [None; PORTS],
            host_completion,
            consumed_host_completion: false,
            host_request: None,
            maximum_work: self.node_specs[node].maximum_step_work,
            work: 0,
            fault: None,
        })
    }

    fn apply_step(
        &mut self,
        node: usize,
        outcome: StepOutcome,
        io: StepIo<PORTS>,
    ) -> Result<(), SchedulerError> {
        let staged = io.staged();
        match outcome {
            StepOutcome::Progress if !staged => return Err(SchedulerError::FalseProgress),
            StepOutcome::Await if staged => return Err(SchedulerError::FalseProgress),
            StepOutcome::Yield if staged || io.work != io.maximum_work => {
                return Err(SchedulerError::FalseProgress);
            }
            StepOutcome::Fail(code) => return Err(SchedulerError::OperationFailed(code)),
            _ => {}
        }

        if matches!(outcome, StepOutcome::Progress | StepOutcome::Complete) {
            let mut evidence_records =
                self.commit_event_count(node, &io.consumed, &io.consumed_closed, &io.outputs)?;
            if io.host_request.is_some() {
                evidence_records = evidence_records
                    .checked_add(1)
                    .ok_or(SchedulerError::InvalidPlan)?;
            }
            if matches!(outcome, StepOutcome::Complete) {
                if io.host_request.is_some() {
                    return Err(SchedulerError::InvalidHostOperationAccess);
                }
                evidence_records = evidence_records
                    .checked_add(self.output_route_count(node)?)
                    .and_then(|value| value.checked_add(1))
                    .ok_or(SchedulerError::InvalidPlan)?;
            }
            self.ensure_evidence_capacity(evidence_records)?;
            self.commit(node, io.staged_step())?;
        }
        match outcome {
            StepOutcome::Progress => self.ready[node] = io.host_request.is_none(),
            StepOutcome::Yield => self.ready[node] = true,
            StepOutcome::Await => self.ready[node] = false,
            StepOutcome::Complete => {
                self.completed[node] = true;
                self.ready[node] = false;
                self.close_outputs(node)?;
                self.evidence.record(
                    NodeId(as_u16(node)?),
                    None,
                    None,
                    KernelEventKind::OperationCompleted,
                )?;
            }
            StepOutcome::Fail(_) => unreachable!(),
        }
        Ok(())
    }

    fn commit(&mut self, node: usize, staged: StagedStep<PORTS>) -> Result<(), SchedulerError> {
        let StagedStep {
            consumed,
            retained_inputs,
            consumed_closed,
            outputs,
            discards,
            consumed_host_completion,
            host_request,
        } = staged;
        let mut retained_values = [None; PORTS];
        for (port, retained) in retained_inputs.iter().copied().enumerate() {
            if retained {
                let cord = self.node_specs[node].input_cords[port]
                    .ok_or(SchedulerError::InvalidPortAccess)?;
                retained_values[port] = self.peek(usize::from(cord.0))?;
            }
        }
        let admitted_host_request = self.preflight_step(node, &staged, &retained_values)?;

        for (port, consumed) in consumed_closed.iter().copied().enumerate() {
            if consumed {
                self.evidence.record(
                    NodeId(as_u16(node)?),
                    Some(PortId(as_u16(port)?)),
                    None,
                    KernelEventKind::InputClosed,
                )?;
            }
        }

        let mut consumed_values = [None; PORTS];
        for (port, consume) in consumed.iter().copied().enumerate() {
            if !consume {
                continue;
            }
            let cord =
                self.node_specs[node].input_cords[port].ok_or(SchedulerError::InvalidPortAccess)?;
            let value = self.pop(usize::from(cord.0))?;
            consumed_values[port] = Some(value);
            let spec = self.cord_specs[usize::from(cord.0)];
            if let Some((source_node, _)) = spec.source_local() {
                self.ready[usize::from(source_node.0)] = true;
            }
            self.evidence.record(
                NodeId(as_u16(node)?),
                Some(PortId(as_u16(port)?)),
                None,
                KernelEventKind::ValueConsumed,
            )?;
        }

        let consumed_host_value = if consumed_host_completion {
            let slot = self
                .pending_host_operations
                .iter()
                .position(|pending| {
                    pending
                        .and_then(|pending| pending.completion)
                        .is_some_and(|_| {
                            pending
                                .map(|pending| usize::from(pending.request.node.0) == node)
                                .unwrap_or(false)
                        })
                })
                .ok_or(SchedulerError::InvalidHostOperationAccess)?;
            let pending = self.pending_host_operations[slot]
                .take()
                .ok_or(SchedulerError::InvalidHostOperationAccess)?;
            pending
                .completion
                .ok_or(SchedulerError::InvalidHostOperationAccess)?
                .output
                .map(|output| output.value)
        } else {
            None
        };

        let mut handled = [None; PORTS];
        for value in outputs.iter().copied().flatten() {
            if handled.iter().flatten().any(|handled| *handled == value) {
                continue;
            }
            let consumed_references = consumed_values
                .iter()
                .flatten()
                .filter(|candidate| **candidate == value)
                .count()
                + usize::from(consumed_host_value == Some(value));
            let base_references = consumed_references.max(1);
            let target_references =
                self.step_target_count(node, &outputs, host_request, &retained_values, value)?;
            if target_references > base_references {
                for _ in 0..(target_references - base_references) {
                    self.values.retain(value)?;
                }
            } else {
                for _ in 0..(base_references - target_references) {
                    self.values.release(value)?;
                }
            }
            let slot = handled
                .iter_mut()
                .find(|slot| slot.is_none())
                .ok_or(SchedulerError::InvalidPortAccess)?;
            *slot = Some(value);
        }
        if let Some((_, _, input)) = host_request {
            let value = input.value;
            if !outputs.iter().flatten().any(|output| *output == value) {
                let consumed_references = consumed_values
                    .iter()
                    .flatten()
                    .filter(|candidate| **candidate == value)
                    .count()
                    + usize::from(consumed_host_value == Some(value));
                let base_references = consumed_references.max(1);
                if base_references > 1 {
                    for _ in 0..(base_references - 1) {
                        self.values.release(value)?;
                    }
                }
            }
        }
        for (port, value) in consumed_values.iter().copied().enumerate() {
            let Some(value) = value else {
                continue;
            };
            if !outputs.iter().flatten().any(|output| *output == value)
                && host_request.map(|request| request.2.value) != Some(value)
                && !retained_inputs[port]
            {
                self.values.release(value)?;
            }
        }
        if let Some(value) = consumed_host_value {
            if !outputs.iter().flatten().any(|output| *output == value)
                && host_request.map(|request| request.2.value) != Some(value)
            {
                self.values.release(value)?;
            }
        }

        if let (Some((request, operation, input)), Some(binding)) =
            (host_request, admitted_host_request)
        {
            let slot = self
                .pending_host_operations
                .iter_mut()
                .find(|pending| pending.is_none())
                .ok_or(SchedulerError::HostOperationCapacityExceeded)?;
            *slot = Some(PendingHostOperation {
                request: HostOperationRequest {
                    node: NodeId(as_u16(node)?),
                    request,
                    operation,
                    input,
                },
                maximum_output_bytes: binding.maximum_output_bytes,
                dispatched: false,
                completion: None,
            });
            self.last_host_request[node] = Some(request);
            self.evidence.record(
                NodeId(as_u16(node)?),
                None,
                Some(request),
                KernelEventKind::HostOperationRequested,
            )?;
        }

        for discard in discards.iter().copied().flatten() {
            self.values.release(discard)?;
        }

        for (port, value) in outputs.iter().copied().enumerate() {
            let Some(value) = value else {
                continue;
            };
            let targets = self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))?;
            let targets = targets.collect_targets::<ROUTE_TARGETS>()?;
            for target in targets.iter() {
                self.push(usize::from(target.cord.0), value)?;
                if let CordEndpoint::Local { node, .. } = target.sink {
                    self.ready[usize::from(node.0)] = true;
                }
                self.evidence.record(
                    NodeId(as_u16(node)?),
                    Some(PortId(as_u16(port)?)),
                    None,
                    KernelEventKind::ValueRouted,
                )?;
            }
        }
        Ok(())
    }

    fn preflight_step(
        &self,
        node: usize,
        staged: &StagedStep<PORTS>,
        retained_values: &[Option<ValueRef>; PORTS],
    ) -> Result<Option<HostOperationBinding>, SchedulerError> {
        let consumed = &staged.consumed;
        let retained_inputs = &staged.retained_inputs;
        let outputs = &staged.outputs;
        let discards = &staged.discards;
        let consumed_host_completion = staged.consumed_host_completion;
        let host_request = staged.host_request;
        if retained_inputs
            .iter()
            .zip(consumed)
            .any(|(retained, consumed)| *retained && !*consumed)
        {
            return Err(SchedulerError::InvalidPortAccess);
        }
        let completed_pending = self
            .pending_host_operations
            .iter()
            .flatten()
            .find(|pending| {
                usize::from(pending.request.node.0) == node && pending.completion.is_some()
            });
        let available_host_value = completed_pending
            .and_then(|pending| pending.completion)
            .and_then(|outcome| outcome.output)
            .map(|output| output.value);
        let consumed_host_value = if consumed_host_completion {
            completed_pending
                .ok_or(SchedulerError::InvalidHostOperationAccess)?
                .completion
                .and_then(|outcome| outcome.output)
                .map(|output| output.value)
        } else {
            None
        };
        let node_id = NodeId(as_u16(node)?);
        let admitted_host_request = if let Some((request, operation, input)) = host_request {
            if self.last_host_request[node].is_some_and(|last| request <= last)
                || self
                    .pending_host_operations
                    .iter()
                    .flatten()
                    .any(|pending| {
                        pending.request.node == node_id && pending.request.request == request
                    })
            {
                return Err(SchedulerError::HostOperationRequestDuplicate);
            }
            let pending_for_node = self
                .pending_host_operations
                .iter()
                .flatten()
                .any(|pending| usize::from(pending.request.node.0) == node);
            if pending_for_node && !consumed_host_completion {
                return Err(SchedulerError::InvalidHostOperationAccess);
            }
            if !consumed_host_completion && self.pending_host_operations.iter().all(Option::is_some)
            {
                return Err(SchedulerError::HostOperationCapacityExceeded);
            }
            self.values.get(input.value)?;
            let bindings = self
                .host_bindings
                .as_ref()
                .ok_or(SchedulerError::InvalidHostOperationAccess)?;
            Some(bindings.admit(
                NodeId(as_u16(node)?),
                OperationAction::RequestHostOperation {
                    request,
                    operation,
                    input,
                },
            )?)
        } else {
            None
        };
        for (port, value) in outputs.iter().copied().enumerate() {
            let Some(value) = value else {
                continue;
            };
            if available_host_value == Some(value) && !consumed_host_completion {
                return Err(SchedulerError::InvalidHostOperationAccess);
            }
            if discards.iter().flatten().any(|discard| *discard == value) {
                return Err(SchedulerError::InvalidPortAccess);
            }
            self.values.get(value)?;
            for target in self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))?
            {
                let cord = usize::from(target.cord.0);
                let state = self.cords.get(cord).ok_or(SchedulerError::InvalidPlan)?;
                let spec = self
                    .cord_specs
                    .get(cord)
                    .ok_or(SchedulerError::InvalidPlan)?;
                if state.len >= spec.item_capacity {
                    return Err(SchedulerError::QueueCapacityExceeded);
                }
                if value.byte_len > spec.byte_capacity.saturating_sub(state.queued_bytes) {
                    return Err(SchedulerError::QueueByteCapacityExceeded);
                }
            }
        }
        let mut handled = [None; PORTS];
        for value in outputs.iter().copied().flatten() {
            if handled.iter().flatten().any(|handled| *handled == value) {
                continue;
            }
            let consumed_references = consumed
                .iter()
                .copied()
                .enumerate()
                .filter(|(port, is_consumed)| {
                    *is_consumed
                        && self.node_specs[node].input_cords[*port]
                            .and_then(|cord| self.peek(usize::from(cord.0)).ok().flatten())
                            == Some(value)
                })
                .count()
                + usize::from(consumed_host_value == Some(value));
            let input_matches = self.node_specs[node]
                .input_cords
                .iter()
                .flatten()
                .filter(|cord| self.peek(usize::from(cord.0)).ok().flatten() == Some(value))
                .count();
            if input_matches > 0 && consumed_references == 0 {
                return Err(SchedulerError::InvalidPortAccess);
            }
            let base_references = consumed_references.max(1);
            let target_references =
                self.step_target_count(node, outputs, host_request, retained_values, value)?;
            let current = usize::from(self.values.reference_count(value)?);
            if current < consumed_references {
                return Err(SchedulerError::Storage(StorageError::StaleReference));
            }
            let additional = target_references.saturating_sub(base_references);
            if current
                .checked_add(additional)
                .filter(|count| *count <= usize::from(u16::MAX))
                .is_none()
            {
                return Err(SchedulerError::Storage(StorageError::ReferenceOverflow));
            }
            let slot = handled
                .iter_mut()
                .find(|slot| slot.is_none())
                .ok_or(SchedulerError::InvalidPortAccess)?;
            *slot = Some(value);
        }
        if let Some((_, _, input)) = host_request {
            let value = input.value;
            if available_host_value == Some(value) && !consumed_host_completion {
                return Err(SchedulerError::InvalidHostOperationAccess);
            }
            if discards.iter().flatten().any(|discard| *discard == value)
                || retained_values
                    .iter()
                    .flatten()
                    .any(|retained| *retained == value)
            {
                return Err(SchedulerError::InvalidHostOperationAccess);
            }
            if !outputs.iter().flatten().any(|output| *output == value) {
                let consumed_references = consumed
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(port, is_consumed)| {
                        *is_consumed
                            && self.node_specs[node].input_cords[*port]
                                .and_then(|cord| self.peek(usize::from(cord.0)).ok().flatten())
                                == Some(value)
                    })
                    .count()
                    + usize::from(consumed_host_value == Some(value));
                let input_matches = self.node_specs[node]
                    .input_cords
                    .iter()
                    .flatten()
                    .filter(|cord| self.peek(usize::from(cord.0)).ok().flatten() == Some(value))
                    .count();
                if input_matches > consumed_references {
                    return Err(SchedulerError::InvalidHostOperationAccess);
                }
                let current = usize::from(self.values.reference_count(value)?);
                if current < consumed_references {
                    return Err(SchedulerError::Storage(StorageError::StaleReference));
                }
            }
        }
        for discard in discards.iter().copied().flatten() {
            self.values.get(discard)?;
            if retained_values
                .iter()
                .flatten()
                .any(|retained| *retained == discard)
                || self.node_specs[node]
                    .input_cords
                    .iter()
                    .flatten()
                    .any(|cord| self.peek(usize::from(cord.0)).ok().flatten() == Some(discard))
            {
                return Err(SchedulerError::InvalidPortAccess);
            }
        }
        Ok(admitted_host_request)
    }

    fn commit_event_count(
        &self,
        node: usize,
        consumed: &[bool; PORTS],
        consumed_closed: &[bool; PORTS],
        outputs: &[Option<ValueRef>; PORTS],
    ) -> Result<usize, SchedulerError> {
        let consumed = consumed.iter().filter(|value| **value).count();
        let consumed_closed = consumed_closed.iter().filter(|value| **value).count();
        let mut routed = 0_usize;
        for (port, output) in outputs.iter().enumerate() {
            if output.is_some() {
                routed = routed
                    .checked_add(
                        self.routes
                            .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))?
                            .count(),
                    )
                    .ok_or(SchedulerError::InvalidPlan)?;
            }
        }
        consumed
            .checked_add(consumed_closed)
            .and_then(|value| value.checked_add(routed))
            .ok_or(SchedulerError::InvalidPlan)
    }

    fn output_route_count(&self, node: usize) -> Result<usize, SchedulerError> {
        let mut count = 0_usize;
        for port in 0..PORTS {
            if let Ok(routes) = self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))
            {
                count = count
                    .checked_add(routes.count())
                    .ok_or(SchedulerError::InvalidPlan)?;
            }
        }
        Ok(count)
    }

    fn ensure_evidence_capacity(&self, additional: usize) -> Result<(), SchedulerError> {
        let additional_items =
            u16::try_from(additional).map_err(|_| SchedulerError::InvalidPlan)?;
        if self
            .evidence
            .len()
            .checked_add(additional_items)
            .filter(|len| *len <= self.evidence.item_capacity())
            .is_none()
        {
            return Err(SchedulerError::Evidence(
                EvidenceError::ItemCapacityExceeded,
            ));
        }
        let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>())
            .map_err(|_| SchedulerError::InvalidPlan)?
            .checked_mul(u32::from(additional_items))
            .ok_or(SchedulerError::InvalidPlan)?;
        if self
            .evidence
            .used_bytes()
            .checked_add(charge)
            .filter(|bytes| *bytes <= self.evidence.byte_capacity())
            .is_none()
        {
            return Err(SchedulerError::Evidence(
                EvidenceError::ByteCapacityExceeded,
            ));
        }
        Ok(())
    }

    fn step_target_count(
        &self,
        node: usize,
        outputs: &[Option<ValueRef>; PORTS],
        host_request: Option<(RequestId, HostOperationId, BoundedValueRef)>,
        retained_values: &[Option<ValueRef>; PORTS],
        value: ValueRef,
    ) -> Result<usize, SchedulerError> {
        let mut count = 0_usize;
        for (port, output) in outputs.iter().copied().enumerate() {
            if output != Some(value) {
                continue;
            }
            count = count
                .checked_add(
                    self.routes
                        .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))?
                        .count(),
                )
                .ok_or(SchedulerError::InvalidPlan)?;
        }
        if host_request.map(|request| request.2.value) == Some(value) {
            count = count.checked_add(1).ok_or(SchedulerError::InvalidPlan)?;
        }
        count = count
            .checked_add(
                retained_values
                    .iter()
                    .flatten()
                    .filter(|retained| **retained == value)
                    .count(),
            )
            .ok_or(SchedulerError::InvalidPlan)?;
        Ok(count)
    }

    fn close_outputs(&mut self, node: usize) -> Result<(), SchedulerError> {
        for port in 0..PORTS {
            let Ok(targets) = self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))
            else {
                continue;
            };
            let targets = targets.collect_targets::<ROUTE_TARGETS>()?;
            for target in targets.iter() {
                let cord = usize::from(target.cord.0);
                self.cords[cord].producer_closed = true;
                if let CordEndpoint::Local { node, .. } = target.sink {
                    self.ready[usize::from(node.0)] = true;
                } else {
                    self.evidence.record(
                        NodeId(as_u16(node)?),
                        Some(PortId(as_u16(port)?)),
                        None,
                        KernelEventKind::RemoteOutputClosed,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn peek(&self, cord: usize) -> Result<Option<ValueRef>, SchedulerError> {
        let spec = *self
            .cord_specs
            .get(cord)
            .ok_or(SchedulerError::InvalidPlan)?;
        let state = *self.cords.get(cord).ok_or(SchedulerError::InvalidPlan)?;
        if state.len == 0 {
            return Ok(None);
        }
        let offset = state.head % spec.item_capacity;
        let slot = usize::from(spec.slot_start + offset);
        self.queue_slots
            .get(slot)
            .copied()
            .ok_or(SchedulerError::InvalidPlan)
    }

    fn pop(&mut self, cord: usize) -> Result<ValueRef, SchedulerError> {
        let spec = self.cord_specs[cord];
        let state = &mut self.cords[cord];
        if state.len == 0 {
            return Err(SchedulerError::InvalidPortAccess);
        }
        let offset = state.head % spec.item_capacity;
        let slot = usize::from(spec.slot_start + offset);
        let value = self.queue_slots[slot]
            .take()
            .ok_or(SchedulerError::InvalidPlan)?;
        state.head = (state.head + 1) % spec.item_capacity;
        state.len -= 1;
        state.queued_bytes -= value.byte_len;
        Ok(value)
    }

    fn push(&mut self, cord: usize, value: ValueRef) -> Result<(), SchedulerError> {
        let spec = self.cord_specs[cord];
        let state = &mut self.cords[cord];
        if state.len >= spec.item_capacity {
            return Err(SchedulerError::QueueCapacityExceeded);
        }
        let offset = (state.head + state.len) % spec.item_capacity;
        let slot = usize::from(spec.slot_start + offset);
        if self.queue_slots[slot].is_some() {
            return Err(SchedulerError::InvalidPlan);
        }
        self.queue_slots[slot] = Some(value);
        state.len += 1;
        state.queued_bytes += value.byte_len;
        Ok(())
    }
}

fn validate_plan<
    const NODES: usize,
    const CORDS: usize,
    const PORTS: usize,
    const QUEUE_SLOTS: usize,
    const ROUTE_SLOTS: usize,
    const ROUTE_TARGETS: usize,
>(
    nodes: &[NodeSpec<PORTS>; NODES],
    cords: &[CordSpec; CORDS],
    routes: &FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
) -> Result<(), SchedulerError> {
    for (node_index, node) in nodes.iter().enumerate() {
        if node.maximum_step_work == 0 {
            return Err(SchedulerError::InvalidPlan);
        }
        for (port, cord) in node.input_cords.iter().copied().enumerate() {
            let Some(cord) = cord else {
                continue;
            };
            let spec = cords
                .get(usize::from(cord.0))
                .ok_or(SchedulerError::InvalidPlan)?;
            if spec.sink_local() != Some((NodeId(as_u16(node_index)?), PortId(as_u16(port)?))) {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        for port in 0..PORTS {
            let Ok(targets) = routes.route(NodeId(as_u16(node_index)?), PortId(as_u16(port)?))
            else {
                continue;
            };
            let mut seen = [false; CORDS];
            for target in targets {
                let cord = usize::from(target.cord.0);
                let spec = cords.get(cord).ok_or(SchedulerError::InvalidPlan)?;
                if seen[cord]
                    || spec.source_local()
                        != Some((NodeId(as_u16(node_index)?), PortId(as_u16(port)?)))
                    || spec.sink != target.sink
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                seen[cord] = true;
            }
        }
    }
    for (cord_index, cord) in cords.iter().copied().enumerate() {
        if usize::from(cord.cord.0) != cord_index
            || cord.item_capacity == 0
            || cord.byte_capacity == 0
        {
            return Err(SchedulerError::InvalidPlan);
        }
        match (cord.source, cord.sink) {
            (
                CordEndpoint::Local {
                    node: source_node,
                    port: source_port,
                },
                CordEndpoint::Local {
                    node: sink_node,
                    port: sink_port,
                },
            ) => {
                if usize::from(source_node.0) >= NODES
                    || usize::from(sink_node.0) >= NODES
                    || usize::from(source_port.0) >= PORTS
                    || usize::from(sink_port.0) >= PORTS
                {
                    return Err(SchedulerError::InvalidPlan);
                }
            }
            (
                CordEndpoint::Local {
                    node: source_node,
                    port: source_port,
                },
                CordEndpoint::Remote(_),
            ) => {
                if usize::from(source_node.0) >= NODES || usize::from(source_port.0) >= PORTS {
                    return Err(SchedulerError::InvalidPlan);
                }
            }
            (
                CordEndpoint::Remote(_),
                CordEndpoint::Local {
                    node: sink_node,
                    port: sink_port,
                },
            ) => {
                if usize::from(sink_node.0) >= NODES || usize::from(sink_port.0) >= PORTS {
                    return Err(SchedulerError::InvalidPlan);
                }
            }
            (CordEndpoint::Remote(_), CordEndpoint::Remote(_)) => {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        let end = usize::from(cord.slot_start)
            .checked_add(usize::from(cord.item_capacity))
            .ok_or(SchedulerError::InvalidPlan)?;
        if end > QUEUE_SLOTS {
            return Err(SchedulerError::InvalidPlan);
        }
        if let Some((sink_node, sink_port)) = cord.sink_local() {
            if nodes[usize::from(sink_node.0)].input_cords[usize::from(sink_port.0)]
                != Some(cord.cord)
            {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        if let Some((source_node, source_port)) = cord.source_local() {
            let routed = routes.route(source_node, source_port)?.any(|target| {
                target
                    == RouteTarget {
                        cord: cord.cord,
                        sink: cord.sink,
                    }
            });
            if !routed {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        for other in &cords[..cord_index] {
            if cord
                .remote_endpoint()
                .zip(other.remote_endpoint())
                .is_some_and(|(left, right)| left == right)
            {
                return Err(SchedulerError::InvalidPlan);
            }
            let other_end = usize::from(other.slot_start) + usize::from(other.item_capacity);
            if usize::from(cord.slot_start) < other_end && usize::from(other.slot_start) < end {
                return Err(SchedulerError::InvalidPlan);
            }
        }
    }
    Ok(())
}

fn as_u16(value: usize) -> Result<u16, SchedulerError> {
    u16::try_from(value).map_err(|_| SchedulerError::InvalidPlan)
}

struct TargetBuffer<const CAPACITY: usize> {
    items: [Option<RouteTarget>; CAPACITY],
    len: usize,
}

impl<const CAPACITY: usize> TargetBuffer<CAPACITY> {
    fn iter(&self) -> impl Iterator<Item = RouteTarget> + '_ {
        self.items[..self.len].iter().copied().flatten()
    }
}

trait CollectTargets: Iterator<Item = RouteTarget> + Sized {
    fn collect_targets<const CAPACITY: usize>(
        self,
    ) -> Result<TargetBuffer<CAPACITY>, SchedulerError> {
        let mut buffer = TargetBuffer {
            items: [None; CAPACITY],
            len: 0,
        };
        for target in self {
            if buffer.len >= CAPACITY {
                return Err(SchedulerError::InvalidPlan);
            }
            buffer.items[buffer.len] = Some(target);
            buffer.len += 1;
        }
        Ok(buffer)
    }
}

impl<I: Iterator<Item = RouteTarget>> CollectTargets for I {}

#[cfg(test)]
mod tests;
