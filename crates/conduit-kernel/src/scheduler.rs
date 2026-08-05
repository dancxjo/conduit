//! Fixed-capacity deterministic scheduler over the port-aware kernel contract.

use crate::{
    CordId, EvidenceError, EvidenceSink, FixedRoutes, KernelEventKind, NodeId, PortId,
    ProtocolError, RouteTarget, StorageError, ValueRef, ValueStorage,
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
        self.consumed.iter().any(|value| *value) || self.outputs.iter().any(Option::is_some)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerStatus {
    Progress { node: NodeId },
    Idle,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    InvalidPlan,
    InvalidPortAccess,
    OutputBlocked,
    QueueCapacityExceeded,
    QueueByteCapacityExceeded,
    StepWorkExceeded,
    FalseProgress,
    DecisionLimitExceeded,
    OperationFailed(u16),
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
> where
    D: StepOperation<PORTS>,
    S: ValueStorage,
    E: EvidenceSink,
{
    node_specs: [NodeSpec<PORTS>; NODES],
    cord_specs: [CordSpec; CORDS],
    routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
    drivers: [D; NODES],
    values: S,
    evidence: E,
    cords: [CordState; CORDS],
    queue_slots: [Option<ValueRef>; QUEUE_SLOTS],
    ready: [bool; NODES],
    completed: [bool; NODES],
    cursor: usize,
    decisions: u32,
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
    > FixedScheduler<D, S, E, NODES, CORDS, PORTS, QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS>
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
            drivers,
            values,
            evidence,
            cords: [CordState::EMPTY; CORDS],
            queue_slots: [None; QUEUE_SLOTS],
            ready: [true; NODES],
            completed: [false; NODES],
            cursor: 0,
            decisions: 0,
        })
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
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
            if matches!(outcome, StepOutcome::Complete) {
                evidence_records = evidence_records
                    .checked_add(self.output_route_count(node)?)
                    .and_then(|value| value.checked_add(1))
                    .ok_or(SchedulerError::InvalidPlan)?;
            }
            self.ensure_evidence_capacity(evidence_records)?;
            self.commit(node, io.consumed, io.outputs)?;
        }
        match outcome {
            StepOutcome::Progress | StepOutcome::Yield => self.ready[node] = true,
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
    ) -> Result<(), SchedulerError> {
        self.preflight_outputs(node, &consumed, &outputs)?;

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

        let mut handled = [None; PORTS];
        for value in outputs.iter().copied().flatten() {
            if handled.iter().flatten().any(|handled| *handled == value) {
                continue;
            }
            let consumed_references = consumed_values
                .iter()
                .flatten()
                .filter(|candidate| **candidate == value)
                .count();
            let base_references = consumed_references.max(1);
            let target_references = self.output_target_count(node, &outputs, value)?;
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
        for value in consumed_values.iter().copied().flatten() {
            if !outputs.iter().flatten().any(|output| *output == value) {
                self.values.release(value)?;
            }
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

    fn preflight_outputs(
        &self,
        node: usize,
        consumed: &[bool; PORTS],
        outputs: &[Option<ValueRef>; PORTS],
    ) -> Result<(), SchedulerError> {
        for (port, value) in outputs.iter().copied().enumerate() {
            let Some(value) = value else {
                continue;
            };
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
                .count();
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
            let target_references = self.output_target_count(node, outputs, value)?;
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
        Ok(())
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

    fn output_target_count(
        &self,
        node: usize,
        outputs: &[Option<ValueRef>; PORTS],
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
        CordId, EvidenceQuery, EvidenceSink, FixedEvidenceLog, FixedRoutes, FixedValueStore,
        KernelEventKind, NodeId, PortId, RouteRange, RouteTarget, ValueRef, ValueStorage,
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
