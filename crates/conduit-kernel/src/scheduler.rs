//! Fixed-capacity deterministic scheduler over the port-aware kernel contract.

use crate::{
    BoundedValueRef, CordId, EvidenceError, EvidenceSink, FixedHostOperationBindings, FixedRoutes,
    HostOperationBinding, HostOperationId, HostOperationOutcome, KernelEventKind, NodeId,
    OperationAction, PortId, ProtocolError, RequestId, RouteTarget, StorageError, ValueRef,
    ValueStorage,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeSpec<const PORTS: usize> {
    /// Exact inbound cord for each input-port ordinal.
    pub input_cords: [Option<CordId>; PORTS],
    pub maximum_step_work: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CordSpec {
    pub cord: CordId,
    pub source_node: NodeId,
    pub source_port: PortId,
    pub sink_node: NodeId,
    pub sink_port: PortId,
    pub slot_start: u16,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepOutcome {
    Progress,
    Await,
    Yield,
    Complete,
    Fail(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostOperationRequest {
    pub node: NodeId,
    pub request: RequestId,
    pub operation: HostOperationId,
    pub input: BoundedValueRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingHostOperation {
    request: HostOperationRequest,
    maximum_output_bytes: u32,
    dispatched: bool,
    completion: Option<HostOperationOutcome>,
}

pub trait StepOperation<const PORTS: usize> {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome;
    fn cancel(&mut self) {}
}

pub struct StepIo<const PORTS: usize> {
    inputs: [Option<ValueRef>; PORTS],
    input_closed: [bool; PORTS],
    output_maximum_bytes: [Option<u32>; PORTS],
    consumed: [bool; PORTS],
    outputs: [Option<ValueRef>; PORTS],
    host_completion: Option<(RequestId, HostOperationOutcome)>,
    consumed_host_completion: bool,
    host_request: Option<(RequestId, HostOperationId, BoundedValueRef)>,
    maximum_work: u16,
    work: u16,
    fault: Option<SchedulerError>,
}

impl<const PORTS: usize> StepIo<PORTS> {
    pub fn input(&self, port: PortId) -> Option<ValueRef> {
        self.inputs.get(usize::from(port.0)).copied().flatten()
    }

    pub fn input_closed(&self, port: PortId) -> bool {
        self.input_closed
            .get(usize::from(port.0))
            .copied()
            .unwrap_or(true)
    }

    pub fn consume(&mut self, port: PortId) -> Result<ValueRef, SchedulerError> {
        self.charge_work(1)?;
        let index = usize::from(port.0);
        let value = self
            .inputs
            .get(index)
            .copied()
            .flatten()
            .ok_or(SchedulerError::InvalidPortAccess)?;
        if self.consumed.get(index).copied().unwrap_or(true) {
            return self.fail(SchedulerError::InvalidPortAccess);
        }
        self.consumed[index] = true;
        Ok(value)
    }

    pub fn output_ready(&self, port: PortId) -> bool {
        self.output_maximum_bytes
            .get(usize::from(port.0))
            .copied()
            .flatten()
            .is_some()
    }

    pub fn send(&mut self, port: PortId, value: ValueRef) -> Result<(), SchedulerError> {
        self.charge_work(1)?;
        let index = usize::from(port.0);
        let maximum = self
            .output_maximum_bytes
            .get(index)
            .copied()
            .flatten()
            .ok_or(SchedulerError::OutputBlocked)?;
        if value.byte_len > maximum || self.outputs.get(index).is_none() {
            return self.fail(SchedulerError::OutputBlocked);
        }
        if self.outputs[index].is_some() {
            return self.fail(SchedulerError::InvalidPortAccess);
        }
        self.outputs[index] = Some(value);
        Ok(())
    }

    pub fn host_completion(&self) -> Option<(RequestId, HostOperationOutcome)> {
        self.host_completion
    }

    pub fn consume_host_completion(
        &mut self,
    ) -> Result<(RequestId, HostOperationOutcome), SchedulerError> {
        self.charge_work(1)?;
        if self.consumed_host_completion {
            return self.fail(SchedulerError::InvalidHostOperationAccess);
        }
        let completion = self
            .host_completion
            .ok_or(SchedulerError::InvalidHostOperationAccess)?;
        self.consumed_host_completion = true;
        Ok(completion)
    }

    pub fn request_host_operation(
        &mut self,
        request: RequestId,
        operation: HostOperationId,
        input: BoundedValueRef,
    ) -> Result<(), SchedulerError> {
        self.charge_work(1)?;
        if self.host_request.is_some() {
            return self.fail(SchedulerError::InvalidHostOperationAccess);
        }
        self.host_request = Some((request, operation, input));
        Ok(())
    }

    pub fn charge_work(&mut self, units: u16) -> Result<(), SchedulerError> {
        let work = self
            .work
            .checked_add(units)
            .ok_or(SchedulerError::StepWorkExceeded)?;
        if work > self.maximum_work {
            return self.fail(SchedulerError::StepWorkExceeded);
        }
        self.work = work;
        Ok(())
    }

    pub fn exhaust_work_budget(&mut self) {
        self.work = self.maximum_work;
    }

    fn fail<T>(&mut self, error: SchedulerError) -> Result<T, SchedulerError> {
        if self.fault.is_none() {
            self.fault = Some(error);
        }
        Err(error)
    }

    fn staged(&self) -> bool {
        self.consumed.iter().any(|value| *value)
            || self.outputs.iter().any(Option::is_some)
            || self.consumed_host_completion
            || self.host_request.is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerStatus {
    Progress { node: NodeId },
    Idle,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    InvalidPlan,
    InvalidPortAccess,
    InvalidHostOperationAccess,
    OutputBlocked,
    QueueCapacityExceeded,
    QueueByteCapacityExceeded,
    StepWorkExceeded,
    FalseProgress,
    DecisionLimitExceeded,
    OperationFailed(u16),
    HostOperationCapacityExceeded,
    HostOperationRequestDuplicate,
    HostOperationCompletionRejected,
    HostOperationOutputExceeded,
    Cancelled,
    Storage(StorageError),
    Evidence(EvidenceError),
    Routing(ProtocolError),
}

impl From<StorageError> for SchedulerError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

impl From<EvidenceError> for SchedulerError {
    fn from(value: EvidenceError) -> Self {
        Self::Evidence(value)
    }
}

impl From<ProtocolError> for SchedulerError {
    fn from(value: ProtocolError) -> Self {
        Self::Routing(value)
    }
}

#[derive(Clone, Copy, Debug)]
struct CordState {
    head: u16,
    len: u16,
    queued_bytes: u32,
    producer_closed: bool,
}

impl CordState {
    const EMPTY: Self = Self {
        head: 0,
        len: 0,
        queued_bytes: 0,
        producer_closed: false,
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
    last_host_request: Option<RequestId>,
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
            last_host_request: None,
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
                    .map(|pending| pending.request.request == request)
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
        {
            return Err(SchedulerError::InvalidHostOperationAccess);
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
            if self.ready[node] && !self.completed[node] {
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
            outputs: [None; PORTS],
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
            let mut evidence_records = self.commit_event_count(node, &io.consumed, &io.outputs)?;
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
            self.commit(
                node,
                io.consumed,
                io.outputs,
                io.consumed_host_completion,
                io.host_request,
            )?;
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

    fn commit(
        &mut self,
        node: usize,
        consumed: [bool; PORTS],
        outputs: [Option<ValueRef>; PORTS],
        consumed_host_completion: bool,
        host_request: Option<(RequestId, HostOperationId, BoundedValueRef)>,
    ) -> Result<(), SchedulerError> {
        let admitted_host_request = self.preflight_step(
            node,
            &consumed,
            &outputs,
            consumed_host_completion,
            host_request,
        )?;

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
            self.ready[usize::from(spec.source_node.0)] = true;
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
            let target_references = self.step_target_count(node, &outputs, host_request, value)?;
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
        for value in consumed_values.iter().copied().flatten() {
            if !outputs.iter().flatten().any(|output| *output == value)
                && host_request.map(|request| request.2.value) != Some(value)
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
            self.last_host_request = Some(request);
            self.evidence.record(
                NodeId(as_u16(node)?),
                None,
                Some(request),
                KernelEventKind::HostOperationRequested,
            )?;
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
                self.ready[usize::from(target.sink_node.0)] = true;
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
        consumed: &[bool; PORTS],
        outputs: &[Option<ValueRef>; PORTS],
        consumed_host_completion: bool,
        host_request: Option<(RequestId, HostOperationId, BoundedValueRef)>,
    ) -> Result<Option<HostOperationBinding>, SchedulerError> {
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
        let admitted_host_request = if let Some((request, operation, input)) = host_request {
            if self.last_host_request.is_some_and(|last| request <= last)
                || self
                    .pending_host_operations
                    .iter()
                    .flatten()
                    .any(|pending| pending.request.request == request)
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
            let target_references = self.step_target_count(node, outputs, host_request, value)?;
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
        Ok(admitted_host_request)
    }

    fn commit_event_count(
        &self,
        node: usize,
        consumed: &[bool; PORTS],
        outputs: &[Option<ValueRef>; PORTS],
    ) -> Result<usize, SchedulerError> {
        let consumed = consumed.iter().filter(|value| **value).count();
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
            .checked_add(routed)
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
                self.ready[usize::from(target.sink_node.0)] = true;
                if self.cords[cord].len == 0 {
                    self.evidence.record(
                        target.sink_node,
                        Some(target.sink_port),
                        None,
                        KernelEventKind::InputClosed,
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
            if usize::from(spec.sink_node.0) != node_index || usize::from(spec.sink_port.0) != port
            {
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
                    || usize::from(spec.source_node.0) != node_index
                    || usize::from(spec.source_port.0) != port
                    || spec.sink_node != target.sink_node
                    || spec.sink_port != target.sink_port
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                seen[cord] = true;
            }
        }
    }
    for (cord_index, cord) in cords.iter().copied().enumerate() {
        if usize::from(cord.cord.0) != cord_index
            || usize::from(cord.source_node.0) >= NODES
            || usize::from(cord.sink_node.0) >= NODES
            || usize::from(cord.source_port.0) >= PORTS
            || usize::from(cord.sink_port.0) >= PORTS
            || cord.item_capacity == 0
            || cord.byte_capacity == 0
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let end = usize::from(cord.slot_start)
            .checked_add(usize::from(cord.item_capacity))
            .ok_or(SchedulerError::InvalidPlan)?;
        if end > QUEUE_SLOTS {
            return Err(SchedulerError::InvalidPlan);
        }
        if nodes[usize::from(cord.sink_node.0)].input_cords[usize::from(cord.sink_port.0)]
            != Some(cord.cord)
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let routed = routes
            .route(cord.source_node, cord.source_port)?
            .any(|target| {
                target
                    == RouteTarget {
                        cord: cord.cord,
                        sink_node: cord.sink_node,
                        sink_port: cord.sink_port,
                    }
            });
        if !routed {
            return Err(SchedulerError::InvalidPlan);
        }
        for other in &cords[..cord_index] {
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
mod tests {
    use super::{
        CordSpec, FixedScheduler, NodeSpec, SchedulerStatus, StepIo, StepOperation, StepOutcome,
    };
    use crate::{
        BoundedValueRef, CordId, EvidenceQuery, EvidenceSink, Failure, FailureCode,
        FixedEvidenceLog, FixedHostOperationBindings, FixedRoutes, FixedValueStore,
        HostOperationBinding, HostOperationDisposition, HostOperationId, HostOperationOutcome,
        KernelEventKind, NodeId, PortId, ProtocolError, RequestId, RouteRange, RouteTarget,
        ValueRef, ValueStorage,
    };

    const NODES: usize = 6;
    const CORDS: usize = 5;
    const PORTS: usize = 2;

    #[derive(Clone, Copy, Debug)]
    enum Driver {
        Source {
            values: [Option<ValueRef>; 4],
            next: usize,
        },
        Tee,
        Filter,
        Latest,
        Sink {
            seen: [Option<ValueRef>; 4],
            len: usize,
            stall: bool,
        },
        BlockedSink {
            cancelled: bool,
        },
    }

    impl StepOperation<PORTS> for Driver {
        fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
            match self {
                Self::Source { values, next } => {
                    let Some(value) = values.get(*next).copied().flatten() else {
                        return StepOutcome::Complete;
                    };
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.send(PortId(0), value).unwrap();
                    *next += 1;
                    StepOutcome::Progress
                }
                Self::Tee => {
                    if let Some(value) = io.input(PortId(0)) {
                        if !io.output_ready(PortId(0)) || !io.output_ready(PortId(1)) {
                            return StepOutcome::Await;
                        }
                        io.consume(PortId(0)).unwrap();
                        io.send(PortId(0), value).unwrap();
                        io.send(PortId(1), value).unwrap();
                        StepOutcome::Progress
                    } else if io.input_closed(PortId(0)) {
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Await
                    }
                }
                Self::Filter => {
                    if let Some(value) = io.input(PortId(0)) {
                        if value.slot % 2 == 0 && !io.output_ready(PortId(0)) {
                            return StepOutcome::Await;
                        }
                        io.consume(PortId(0)).unwrap();
                        if value.slot % 2 == 0 {
                            io.send(PortId(0), value).unwrap();
                        }
                        StepOutcome::Progress
                    } else if io.input_closed(PortId(0)) {
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Await
                    }
                }
                Self::Latest => {
                    if let Some(value) = io.input(PortId(0)) {
                        if !io.output_ready(PortId(0)) {
                            return StepOutcome::Await;
                        }
                        io.consume(PortId(0)).unwrap();
                        io.send(PortId(0), value).unwrap();
                        StepOutcome::Progress
                    } else if io.input_closed(PortId(0)) {
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Await
                    }
                }
                Self::Sink { seen, len, stall } => {
                    if *stall && io.input(PortId(0)).is_some() {
                        *stall = false;
                        io.exhaust_work_budget();
                        return StepOutcome::Yield;
                    }
                    if let Some(value) = io.input(PortId(0)) {
                        io.consume(PortId(0)).unwrap();
                        seen[*len] = Some(value);
                        *len += 1;
                        *stall = true;
                        StepOutcome::Progress
                    } else if io.input_closed(PortId(0)) {
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Await
                    }
                }
                Self::BlockedSink { .. } => StepOutcome::Await,
            }
        }

        fn cancel(&mut self) {
            if let Self::BlockedSink { cancelled } = self {
                *cancelled = true;
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum HostDriver {
        Source {
            value: Option<ValueRef>,
        },
        Effect {
            requested: bool,
            cancelled: bool,
            repeat_request: bool,
        },
        Sink {
            seen: Option<ValueRef>,
        },
    }

    impl StepOperation<PORTS> for HostDriver {
        fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
            match self {
                Self::Source { value } => {
                    let Some(current) = *value else {
                        return StepOutcome::Complete;
                    };
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.send(PortId(0), current).unwrap();
                    *value = None;
                    StepOutcome::Progress
                }
                Self::Effect { requested, .. } if !*requested => {
                    let Some(input) = io.input(PortId(0)) else {
                        return StepOutcome::Await;
                    };
                    io.consume(PortId(0)).unwrap();
                    io.request_host_operation(
                        RequestId(7),
                        HostOperationId(0),
                        BoundedValueRef::new(input, 4).unwrap(),
                    )
                    .unwrap();
                    *requested = true;
                    StepOutcome::Progress
                }
                Self::Effect { repeat_request, .. } => {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    assert_eq!(request, RequestId(7));
                    let output = outcome.output.expect("host output").value;
                    if *repeat_request {
                        io.consume_host_completion().unwrap();
                        io.request_host_operation(
                            request,
                            HostOperationId(0),
                            BoundedValueRef::new(output, 4).unwrap(),
                        )
                        .unwrap();
                        return StepOutcome::Progress;
                    }
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion().unwrap();
                    io.send(PortId(0), output).unwrap();
                    StepOutcome::Complete
                }
                Self::Sink { seen } => {
                    if let Some(value) = io.input(PortId(0)) {
                        io.consume(PortId(0)).unwrap();
                        *seen = Some(value);
                        StepOutcome::Progress
                    } else if io.input_closed(PortId(0)) {
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Await
                    }
                }
            }
        }

        fn cancel(&mut self) {
            if let Self::Effect { cancelled, .. } = self {
                *cancelled = true;
            }
        }
    }

    #[test]
    fn multi_value_port_graph_handles_pressure_closure_and_uneven_consumers() {
        let event_charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
        let normalized = execute(
            FixedValueStore::<8, 4>::new(16).unwrap(),
            FixedEvidenceLog::<128>::new(event_charge * 128).unwrap(),
        );
        assert_eq!(normalized.show_a_len, 2);
        assert_eq!(normalized.show_a[..2], [0, 2]);
        assert_eq!(normalized.show_b_len, 4);
        assert_eq!(normalized.show_b, [0, 1, 2, 3]);
        assert_eq!(normalized.used_items, 0);
        assert!(normalized.saw_input_closed);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn hosted_and_fixed_schedulers_have_matching_multi_value_vectors() {
        use crate::{HostedEvidenceLog, HostedValueStore};

        let event_charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
        let fixed = execute(
            FixedValueStore::<8, 4>::new(16).unwrap(),
            FixedEvidenceLog::<128>::new(event_charge * 128).unwrap(),
        );
        let hosted = execute(
            HostedValueStore::new(8, 4, 16).unwrap(),
            HostedEvidenceLog::new(128, event_charge * 128).unwrap(),
        );
        assert_eq!(fixed, hosted);
    }

    #[test]
    fn scheduler_admits_correlates_and_wakes_host_operations() {
        let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
        let normalized = execute_host_operation(
            FixedValueStore::<8, 8>::new(32).unwrap(),
            FixedEvidenceLog::<64>::new(charge * 64).unwrap(),
        );
        assert_eq!(normalized.request, RequestId(7));
        assert_eq!(normalized.operation, HostOperationId(0));
        assert_eq!(normalized.input, [3]);
        assert_eq!(normalized.output_slot, 1);
        assert_eq!(normalized.used_items, 0);
        assert_eq!(normalized.pending, 0);
        assert!(normalized.saw_requested);
        assert!(normalized.saw_completed);
    }

    #[test]
    fn scheduler_rejects_unbound_host_operation_before_consumption_commit() {
        let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
        let mut scheduler = host_scheduler_with_binding_node(
            FixedValueStore::<8, 8>::new(32).unwrap(),
            FixedEvidenceLog::<64>::new(charge * 64).unwrap(),
            NodeId(0),
        );
        scheduler.step().unwrap();
        assert_eq!(scheduler.cords[0].len, 1);
        assert_eq!(
            scheduler.step(),
            Err(super::SchedulerError::Routing(
                ProtocolError::HostOperationMissing
            ))
        );
        assert_eq!(scheduler.cords[0].len, 1);
        assert_eq!(scheduler.values().used_items(), 1);
        assert_eq!(scheduler.pending_host_operation_count(), 0);
    }

    #[test]
    fn scheduler_never_reuses_a_retired_request_identity() {
        let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
        let mut scheduler = host_scheduler(
            FixedValueStore::<8, 8>::new(32).unwrap(),
            FixedEvidenceLog::<64>::new(charge * 64).unwrap(),
        );
        scheduler.step().unwrap();
        scheduler.step().unwrap();
        let request = scheduler.next_host_request().unwrap();
        let output = scheduler.store_host_value(&[4]).unwrap();
        scheduler
            .complete_host_operation(
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(BoundedValueRef::new(output, 4).unwrap()),
                    failure: None,
                },
            )
            .unwrap();
        let HostDriver::Effect { repeat_request, .. } = &mut scheduler.drivers[1] else {
            panic!("effect driver");
        };
        *repeat_request = true;
        scheduler.step().unwrap();
        scheduler.step().unwrap();
        assert_eq!(
            scheduler.step(),
            Err(super::SchedulerError::HostOperationRequestDuplicate)
        );
        assert_eq!(scheduler.pending_host_operation_count(), 1);
        assert_eq!(scheduler.values().used_items(), 1);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn hosted_and_fixed_host_operation_vectors_match() {
        use crate::{HostedEvidenceLog, HostedValueStore};

        let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
        let fixed = execute_host_operation(
            FixedValueStore::<8, 8>::new(32).unwrap(),
            FixedEvidenceLog::<64>::new(charge * 64).unwrap(),
        );
        let hosted = execute_host_operation(
            HostedValueStore::new(8, 8, 32).unwrap(),
            HostedEvidenceLog::new(64, charge * 64).unwrap(),
        );
        assert_eq!(fixed, hosted);
    }

    #[test]
    fn cancellation_rejects_late_host_completion_and_releases_pending_input() {
        let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
        let mut scheduler = host_scheduler(
            FixedValueStore::<8, 8>::new(32).unwrap(),
            FixedEvidenceLog::<64>::new(charge * 64).unwrap(),
        );
        scheduler.step().unwrap();
        scheduler.step().unwrap();
        let request = scheduler.next_host_request().unwrap();
        assert_eq!(request.request, RequestId(7));
        scheduler.cancel().unwrap();
        assert_eq!(scheduler.pending_host_operation_count(), 0);
        assert_eq!(scheduler.values().used_items(), 0);
        assert_eq!(
            scheduler.complete_host_operation(
                RequestId(7),
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Cancelled,
                    output: None,
                    failure: Some(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    }),
                },
            ),
            Err(super::SchedulerError::HostOperationCompletionRejected)
        );
        assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Cancelled);
        let HostDriver::Effect { cancelled, .. } = scheduler.drivers()[1] else {
            panic!("effect driver");
        };
        assert!(cancelled);
        assert!(scheduler
            .evidence()
            .contains_kind(KernelEventKind::RunCancelled));
    }

    #[derive(Debug, Eq, PartialEq)]
    struct HostNormalized {
        request: RequestId,
        operation: HostOperationId,
        input: [u8; 1],
        output_slot: u16,
        decisions: u32,
        evidence_len: u16,
        evidence_bytes: u32,
        used_items: u16,
        pending: usize,
        saw_requested: bool,
        saw_completed: bool,
    }

    fn execute_host_operation<S, E>(values: S, evidence: E) -> HostNormalized
    where
        S: ValueStorage,
        E: EvidenceSink + EvidenceQuery,
    {
        let mut scheduler = host_scheduler(values, evidence);
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { node: NodeId(0) }
        ));
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { node: NodeId(1) }
        ));
        assert_eq!(scheduler.pending_host_operation_count(), 1);
        assert_eq!(
            scheduler.complete_host_operation(
                RequestId(7),
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            ),
            Err(super::SchedulerError::HostOperationCompletionRejected)
        );
        let request = scheduler.next_host_request().unwrap();
        let mut input = [0];
        input.copy_from_slice(scheduler.host_value(request.input.value).unwrap());
        assert_eq!(
            scheduler.complete_host_operation(
                RequestId(8),
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            ),
            Err(super::SchedulerError::HostOperationCompletionRejected)
        );
        let oversized = scheduler.store_host_value(&[0, 1, 2, 3, 4]).unwrap();
        assert_eq!(
            scheduler.complete_host_operation(
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(BoundedValueRef::new(oversized, 5).unwrap()),
                    failure: None,
                },
            ),
            Err(super::SchedulerError::HostOperationOutputExceeded)
        );
        scheduler.discard_host_value(oversized).unwrap();
        let output = scheduler.store_host_value(&[4]).unwrap();
        scheduler
            .complete_host_operation(
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(BoundedValueRef::new(output, 4).unwrap()),
                    failure: None,
                },
            )
            .unwrap();
        assert_eq!(
            scheduler.complete_host_operation(
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            ),
            Err(super::SchedulerError::HostOperationCompletionRejected)
        );
        scheduler.run(32).unwrap();
        let HostDriver::Sink { seen: Some(seen) } = scheduler.drivers()[2] else {
            panic!("host sink");
        };
        HostNormalized {
            request: request.request,
            operation: request.operation,
            input,
            output_slot: seen.slot,
            decisions: scheduler.decisions(),
            evidence_len: scheduler.evidence().len(),
            evidence_bytes: scheduler.evidence().used_bytes(),
            used_items: scheduler.values().used_items(),
            pending: scheduler.pending_host_operation_count(),
            saw_requested: scheduler
                .evidence()
                .contains_kind(KernelEventKind::HostOperationRequested),
            saw_completed: scheduler
                .evidence()
                .contains_kind(KernelEventKind::HostOperationCompleted),
        }
    }

    fn host_scheduler<S, E>(
        values: S,
        evidence: E,
    ) -> FixedScheduler<HostDriver, S, E, 3, 2, 2, 2, 6, 2, 3, 1>
    where
        S: ValueStorage,
        E: EvidenceSink,
    {
        host_scheduler_with_binding_node(values, evidence, NodeId(1))
    }

    fn host_scheduler_with_binding_node<S, E>(
        mut values: S,
        evidence: E,
        binding_node: NodeId,
    ) -> FixedScheduler<HostDriver, S, E, 3, 2, 2, 2, 6, 2, 3, 1>
    where
        S: ValueStorage,
        E: EvidenceSink,
    {
        let input = values.store(&[3]).unwrap();
        let mut routes = FixedRoutes::<6, 2>::new(2);
        for (node, cord, sink) in [(0, 0, 1), (1, 1, 2)] {
            routes
                .install(
                    NodeId(node),
                    PortId(0),
                    RouteRange {
                        start: cord,
                        len: 1,
                    },
                    &[RouteTarget {
                        cord: CordId(cord),
                        sink_node: NodeId(sink),
                        sink_port: PortId(0),
                    }],
                )
                .unwrap();
        }
        routes.seal().unwrap();
        let mut bindings = FixedHostOperationBindings::<3>::new(1);
        bindings
            .install(
                binding_node,
                HostOperationBinding {
                    operation: HostOperationId(0),
                    maximum_input_bytes: 4,
                    maximum_output_bytes: 4,
                },
            )
            .unwrap();
        bindings.seal().unwrap();
        FixedScheduler::new_with_host_operations(
            [
                node([None, None]),
                node([Some(CordId(0)), None]),
                node([Some(CordId(1)), None]),
            ],
            [cord(0, 0, 0, 1, 0), cord(1, 1, 0, 2, 0)],
            routes,
            bindings,
            [
                HostDriver::Source { value: Some(input) },
                HostDriver::Effect {
                    requested: false,
                    cancelled: false,
                    repeat_request: false,
                },
                HostDriver::Sink { seen: None },
            ],
            values,
            evidence,
        )
        .unwrap()
    }

    #[test]
    fn cancellation_releases_queued_and_driver_owned_values_and_is_terminal() {
        let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
        let normalized = execute_cancellation(
            FixedValueStore::<4, 4>::new(8).unwrap(),
            FixedEvidenceLog::<16>::new(charge * 16).unwrap(),
        );
        assert_eq!(normalized.used_items, 0);
        assert!(normalized.driver_cancelled);
        assert_eq!(normalized.status, SchedulerStatus::Cancelled);
        assert!(normalized.saw_cancellation_requested);
        assert!(normalized.saw_run_cancelled);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn hosted_and_fixed_cancellation_vectors_match() {
        use crate::{HostedEvidenceLog, HostedValueStore};

        let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>()).unwrap();
        let fixed = execute_cancellation(
            FixedValueStore::<4, 4>::new(8).unwrap(),
            FixedEvidenceLog::<16>::new(charge * 16).unwrap(),
        );
        let hosted = execute_cancellation(
            HostedValueStore::new(4, 4, 8).unwrap(),
            HostedEvidenceLog::new(16, charge * 16).unwrap(),
        );
        assert_eq!(fixed, hosted);
    }

    #[derive(Debug, Eq, PartialEq)]
    struct CancellationNormalized {
        used_items: u16,
        evidence_len: u16,
        evidence_bytes: u32,
        driver_cancelled: bool,
        status: SchedulerStatus,
        saw_cancellation_requested: bool,
        saw_run_cancelled: bool,
    }

    fn execute_cancellation<S, E>(mut values: S, evidence: E) -> CancellationNormalized
    where
        S: ValueStorage,
        E: EvidenceSink + EvidenceQuery,
    {
        let source_values = [
            Some(values.store(&[0]).unwrap()),
            Some(values.store(&[1]).unwrap()),
            None,
            None,
        ];
        let mut routes = FixedRoutes::<2, 1>::new(1);
        routes
            .install(
                NodeId(0),
                PortId(0),
                RouteRange { start: 0, len: 1 },
                &[RouteTarget {
                    cord: CordId(0),
                    sink_node: NodeId(1),
                    sink_port: PortId(0),
                }],
            )
            .unwrap();
        routes.seal().unwrap();
        let mut scheduler = FixedScheduler::<_, _, _, 2, 1, 2, 1, 2, 1>::new(
            [
                NodeSpec {
                    input_cords: [None, None],
                    maximum_step_work: 1,
                },
                NodeSpec {
                    input_cords: [Some(CordId(0)), None],
                    maximum_step_work: 1,
                },
            ],
            [CordSpec {
                cord: CordId(0),
                source_node: NodeId(0),
                source_port: PortId(0),
                sink_node: NodeId(1),
                sink_port: PortId(0),
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
            }],
            routes,
            [
                Driver::Source {
                    values: source_values,
                    next: 0,
                },
                Driver::BlockedSink { cancelled: false },
            ],
            values,
            evidence,
        )
        .unwrap();
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { node: NodeId(0) }
        ));
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { node: NodeId(1) }
        ));
        scheduler.cancel().unwrap();
        let Driver::BlockedSink { cancelled } = scheduler.drivers()[1] else {
            panic!("blocked sink");
        };
        CancellationNormalized {
            used_items: scheduler.values().used_items(),
            evidence_len: scheduler.evidence().len(),
            evidence_bytes: scheduler.evidence().used_bytes(),
            driver_cancelled: cancelled,
            status: scheduler.step().unwrap(),
            saw_cancellation_requested: scheduler
                .evidence()
                .contains_kind(KernelEventKind::CancellationRequested),
            saw_run_cancelled: scheduler
                .evidence()
                .contains_kind(KernelEventKind::RunCancelled),
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Normalized {
        show_a: [u16; 4],
        show_a_len: usize,
        show_b: [u16; 4],
        show_b_len: usize,
        decisions: u32,
        evidence_len: u16,
        evidence_bytes: u32,
        used_items: u16,
        saw_input_closed: bool,
    }

    fn execute<S, E>(mut values: S, evidence: E) -> Normalized
    where
        S: ValueStorage,
        E: EvidenceSink + EvidenceQuery,
    {
        let source_values = [
            Some(values.store(&[0]).unwrap()),
            Some(values.store(&[1]).unwrap()),
            Some(values.store(&[2]).unwrap()),
            Some(values.store(&[3]).unwrap()),
        ];
        let mut routes = FixedRoutes::<{ NODES * PORTS }, CORDS>::new(PORTS as u16);
        for (source_node, source_port, target) in [
            (
                0,
                0,
                RouteTarget {
                    cord: CordId(0),
                    sink_node: NodeId(1),
                    sink_port: PortId(0),
                },
            ),
            (
                1,
                0,
                RouteTarget {
                    cord: CordId(1),
                    sink_node: NodeId(2),
                    sink_port: PortId(0),
                },
            ),
            (
                1,
                1,
                RouteTarget {
                    cord: CordId(2),
                    sink_node: NodeId(3),
                    sink_port: PortId(0),
                },
            ),
            (
                2,
                0,
                RouteTarget {
                    cord: CordId(3),
                    sink_node: NodeId(4),
                    sink_port: PortId(0),
                },
            ),
            (
                3,
                0,
                RouteTarget {
                    cord: CordId(4),
                    sink_node: NodeId(5),
                    sink_port: PortId(0),
                },
            ),
        ] {
            routes
                .install(
                    NodeId(source_node),
                    PortId(source_port),
                    RouteRange {
                        start: target.cord.0,
                        len: 1,
                    },
                    &[target],
                )
                .unwrap();
        }
        routes.seal().unwrap();
        let cords = [
            cord(0, 0, 0, 1, 0),
            cord(1, 1, 0, 2, 0),
            cord(2, 1, 1, 3, 0),
            cord(3, 2, 0, 4, 0),
            cord(4, 3, 0, 5, 0),
        ];
        let nodes = [
            node([None, None]),
            node([Some(CordId(0)), None]),
            node([Some(CordId(1)), None]),
            node([Some(CordId(2)), None]),
            node([Some(CordId(3)), None]),
            node([Some(CordId(4)), None]),
        ];
        let drivers = [
            Driver::Source {
                values: source_values,
                next: 0,
            },
            Driver::Tee,
            Driver::Filter,
            Driver::Latest,
            Driver::Sink {
                seen: [None; 4],
                len: 0,
                stall: true,
            },
            Driver::Sink {
                seen: [None; 4],
                len: 0,
                stall: true,
            },
        ];
        let mut scheduler =
            FixedScheduler::<_, _, _, NODES, CORDS, PORTS, CORDS, { NODES * PORTS }, CORDS>::new(
                nodes, cords, routes, drivers, values, evidence,
            )
            .unwrap();

        scheduler.run(128).unwrap();
        assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Complete);
        let Driver::Sink { seen, len, .. } = &scheduler.drivers()[4] else {
            panic!("show-a sink");
        };
        let mut show_a = [u16::MAX; 4];
        for (index, value) in seen[..*len].iter().enumerate() {
            show_a[index] = value.unwrap().slot;
        }
        let show_a_len = *len;
        let Driver::Sink { seen, len, .. } = &scheduler.drivers()[5] else {
            panic!("show-b sink");
        };
        let mut show_b = [u16::MAX; 4];
        for (index, value) in seen[..*len].iter().enumerate() {
            show_b[index] = value.unwrap().slot;
        }
        Normalized {
            show_a,
            show_a_len,
            show_b,
            show_b_len: *len,
            decisions: scheduler.decisions(),
            evidence_len: scheduler.evidence().len(),
            evidence_bytes: scheduler.evidence().used_bytes(),
            used_items: scheduler.values().used_items(),
            saw_input_closed: scheduler
                .evidence()
                .contains_kind(KernelEventKind::InputClosed),
        }
    }

    fn node(input_cords: [Option<CordId>; PORTS]) -> NodeSpec<PORTS> {
        NodeSpec {
            input_cords,
            maximum_step_work: 3,
        }
    }

    fn cord(
        id: u16,
        source_node: u16,
        source_port: u16,
        sink_node: u16,
        sink_port: u16,
    ) -> CordSpec {
        CordSpec {
            cord: CordId(id),
            source_node: NodeId(source_node),
            source_port: PortId(source_port),
            sink_node: NodeId(sink_node),
            sink_port: PortId(sink_port),
            slot_start: id,
            item_capacity: 1,
            byte_capacity: 4,
        }
    }
}
