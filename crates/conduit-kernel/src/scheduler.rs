//! Fixed-capacity deterministic scheduler over the port-aware kernel contract.

use crate::{
    BoundedValueRef, CordEndpoint, CordId, FixedHostOperationBindings, FixedRoutes,
    HostOperationBinding, HostOperationId, HostOperationOutcome, KernelEventKind, NodeId,
    Operation, OperationAction, OperationInput, PortId, ProtocolError, RemoteEndpointId, RequestId,
    RouteTarget, SignError, SignSink, StorageError, ValueRef, ValueStorage,
};

mod active_capacity;
use active_capacity::validate_active_capacity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeSpec<const PORTS: usize> {
    /// Exact inbound cord for each input-port ordinal.
    pub input_cords: [Option<CordId>; PORTS],
    pub maximum_step_work: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CordSpec {
    pub cord: CordId,
    pub source: CordEndpoint,
    pub sink: CordEndpoint,
    pub slot_start: u16,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CordCapacity {
    pub slot_start: u16,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

impl CordSpec {
    pub const fn local(
        cord: CordId,
        source: (NodeId, PortId),
        sink: (NodeId, PortId),
        capacity: CordCapacity,
    ) -> Self {
        Self {
            cord,
            source: CordEndpoint::local(source.0, source.1),
            sink: CordEndpoint::local(sink.0, sink.1),
            slot_start: capacity.slot_start,
            item_capacity: capacity.item_capacity,
            byte_capacity: capacity.byte_capacity,
        }
    }

    pub const fn remote_egress(
        cord: CordId,
        source: (NodeId, PortId),
        endpoint: RemoteEndpointId,
        capacity: CordCapacity,
    ) -> Self {
        Self {
            cord,
            source: CordEndpoint::local(source.0, source.1),
            sink: CordEndpoint::Remote(endpoint),
            slot_start: capacity.slot_start,
            item_capacity: capacity.item_capacity,
            byte_capacity: capacity.byte_capacity,
        }
    }

    pub const fn remote_ingress(
        cord: CordId,
        endpoint: RemoteEndpointId,
        sink: (NodeId, PortId),
        capacity: CordCapacity,
    ) -> Self {
        Self {
            cord,
            source: CordEndpoint::Remote(endpoint),
            sink: CordEndpoint::local(sink.0, sink.1),
            slot_start: capacity.slot_start,
            item_capacity: capacity.item_capacity,
            byte_capacity: capacity.byte_capacity,
        }
    }

    pub const fn source_local(self) -> Option<(NodeId, PortId)> {
        match self.source {
            CordEndpoint::Local { node, port } => Some((node, port)),
            CordEndpoint::Remote(_) => None,
        }
    }

    pub const fn sink_local(self) -> Option<(NodeId, PortId)> {
        match self.sink {
            CordEndpoint::Local { node, port } => Some((node, port)),
            CordEndpoint::Remote(_) => None,
        }
    }

    pub const fn remote_endpoint(self) -> Option<RemoteEndpointId> {
        match (self.source, self.sink) {
            (CordEndpoint::Remote(endpoint), CordEndpoint::Local { .. })
            | (CordEndpoint::Local { .. }, CordEndpoint::Remote(endpoint)) => Some(endpoint),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteValueOffer {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub sequence: u64,
    pub value: ValueRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteIngressOutcome {
    Accepted { sequence: u64 },
    Full { sequence: u64 },
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

#[derive(Clone, Copy)]
enum AdapterEvent {
    None,
    Value {
        port: PortId,
        value: ValueRef,
    },
    Closed {
        port: PortId,
    },
    HostCompleted {
        request: RequestId,
        outcome: HostOperationOutcome,
    },
}

impl AdapterEvent {
    fn operation_input(self) -> Option<OperationInput> {
        match self {
            Self::None => None,
            Self::Value { port, value } => Some(OperationInput::Value { port, value }),
            Self::Closed { port } => Some(OperationInput::Closed { port }),
            Self::HostCompleted { request, outcome } => {
                Some(OperationInput::HostOperationCompleted { request, outcome })
            }
        }
    }
}

#[derive(Clone, Copy)]
enum AdapterTerminal {
    Continue,
    Complete,
    Fail(u16),
}

#[derive(Clone, Copy)]
struct AdapterTransaction<const PORTS: usize> {
    event: AdapterEvent,
    outputs: [Option<ValueRef>; PORTS],
    host_request: Option<(RequestId, HostOperationId, BoundedValueRef)>,
    retain_resumed_value: bool,
    released_value: Option<ValueRef>,
    terminal: AdapterTerminal,
}

impl<const PORTS: usize> AdapterTransaction<PORTS> {
    fn is_empty_continue(&self) -> bool {
        self.outputs.iter().all(Option::is_none)
            && self.host_request.is_none()
            && !self.retain_resumed_value
            && self.released_value.is_none()
            && matches!(self.terminal, AdapterTerminal::Continue)
    }
}

/// Fixed-capacity adapter from the public operation state machine into one
/// transactional scheduler step.
pub struct OperationDriver<O, const PORTS: usize> {
    operation: O,
    pending: Option<AdapterTransaction<PORTS>>,
    delivered_closed: [bool; PORTS],
    input_cursor: usize,
    protocol_failed: bool,
}

impl<O: Operation, const PORTS: usize> OperationDriver<O, PORTS> {
    pub fn new(mut operation: O) -> Result<Self, SchedulerError> {
        let first = operation.start();
        let mut driver = Self {
            operation,
            pending: None,
            delivered_closed: [false; PORTS],
            input_cursor: 0,
            protocol_failed: false,
        };
        let transaction = driver.collect(AdapterEvent::None, first)?;
        if !transaction.is_empty_continue() {
            driver.pending = Some(transaction);
        }
        Ok(driver)
    }

    pub fn operation(&self) -> &O {
        &self.operation
    }

    fn collect(
        &mut self,
        event: AdapterEvent,
        first: OperationAction,
    ) -> Result<AdapterTransaction<PORTS>, SchedulerError> {
        let mut transaction = AdapterTransaction {
            event,
            outputs: [None; PORTS],
            host_request: None,
            retain_resumed_value: false,
            released_value: None,
            terminal: AdapterTerminal::Continue,
        };
        let mut action = first;
        for _ in 0..PORTS.saturating_add(3) {
            match action {
                OperationAction::Await => return Ok(transaction),
                OperationAction::Emit { port, value } => {
                    let output = transaction
                        .outputs
                        .get_mut(usize::from(port.0))
                        .ok_or(SchedulerError::InvalidPortAccess)?;
                    if output.is_some() {
                        return Err(SchedulerError::OperationProtocolViolation);
                    }
                    *output = Some(value);
                    action = self.operation.advance();
                }
                OperationAction::RequestHostOperation {
                    request,
                    operation,
                    input,
                } => {
                    if transaction.host_request.is_some() {
                        return Err(SchedulerError::OperationProtocolViolation);
                    }
                    transaction.host_request = Some((request, operation, input));
                    return Ok(transaction);
                }
                OperationAction::Complete => {
                    transaction.terminal = AdapterTerminal::Complete;
                    return Ok(transaction);
                }
                OperationAction::Fail(failure) => {
                    transaction.terminal = AdapterTerminal::Fail(failure.detail);
                    return Ok(transaction);
                }
            }
        }
        Err(SchedulerError::OperationProtocolViolation)
    }

    fn next_event(&self, io: &StepIo<PORTS>) -> Option<AdapterEvent> {
        if let Some((request, outcome)) = io.host_completion() {
            return Some(AdapterEvent::HostCompleted { request, outcome });
        }
        for offset in 0..PORTS {
            let port = (self.input_cursor + offset) % PORTS;
            if let Some(value) = io.input(PortId(u16::try_from(port).ok()?)) {
                return Some(AdapterEvent::Value {
                    port: PortId(u16::try_from(port).ok()?),
                    value,
                });
            }
        }
        for offset in 0..PORTS {
            let port = (self.input_cursor + offset) % PORTS;
            if !self.delivered_closed[port] && io.input_closed(PortId(u16::try_from(port).ok()?)) {
                return Some(AdapterEvent::Closed {
                    port: PortId(u16::try_from(port).ok()?),
                });
            }
        }
        None
    }
}

impl<O: Operation, const PORTS: usize> StepOperation<PORTS> for OperationDriver<O, PORTS> {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if self.protocol_failed {
            return StepOutcome::Fail(u16::MAX);
        }
        if self.pending.is_none() {
            let Some(event) = self.next_event(io) else {
                return StepOutcome::Await;
            };
            let Some(input) = event.operation_input() else {
                return StepOutcome::Fail(u16::MAX);
            };
            let action = self.operation.resume(input);
            match self.collect(event, action) {
                Ok(mut transaction) => {
                    transaction.retain_resumed_value = matches!(event, AdapterEvent::Value { .. })
                        && self.operation.retains_resumed_value();
                    transaction.released_value = self.operation.take_released_value();
                    self.pending = Some(transaction);
                }
                Err(_) => {
                    self.protocol_failed = true;
                    return StepOutcome::Fail(u16::MAX);
                }
            }
        }
        let transaction = self.pending.expect("adapter pending transaction");
        for (port, output) in transaction.outputs.iter().enumerate() {
            if output.is_some() && !io.output_ready(PortId(u16::try_from(port).unwrap_or(u16::MAX)))
            {
                return StepOutcome::Await;
            }
        }
        match transaction.event {
            AdapterEvent::None => {}
            AdapterEvent::Value { port, value } => {
                if io.input(port) != Some(value) {
                    return StepOutcome::Fail(u16::MAX);
                }
                let consumed = if transaction.retain_resumed_value {
                    io.take_input(port)
                } else {
                    io.consume(port)
                };
                if consumed.is_err() {
                    return StepOutcome::Fail(u16::MAX);
                }
                self.input_cursor = (usize::from(port.0) + 1) % PORTS;
            }
            AdapterEvent::Closed { port } => {
                self.delivered_closed[usize::from(port.0)] = true;
                self.input_cursor = (usize::from(port.0) + 1) % PORTS;
                if io.consume_closed(port).is_err() {
                    return StepOutcome::Fail(u16::MAX);
                }
            }
            AdapterEvent::HostCompleted { request, .. } => {
                if io.consume_host_completion().map(|completion| completion.0) != Ok(request) {
                    return StepOutcome::Fail(u16::MAX);
                }
            }
        }
        if let Some(value) = transaction.released_value {
            if io.discard(value).is_err() {
                return StepOutcome::Fail(u16::MAX);
            }
        }
        for (port, output) in transaction.outputs.iter().copied().enumerate() {
            if let Some(value) = output {
                if io
                    .send(PortId(u16::try_from(port).unwrap_or(u16::MAX)), value)
                    .is_err()
                {
                    return StepOutcome::Fail(u16::MAX);
                }
            }
        }
        if let Some((request, operation, input)) = transaction.host_request {
            if io
                .request_host_operation(request, operation, input)
                .is_err()
            {
                return StepOutcome::Fail(u16::MAX);
            }
        }
        self.pending = None;
        match transaction.terminal {
            AdapterTerminal::Continue => StepOutcome::Progress,
            AdapterTerminal::Complete => StepOutcome::Complete,
            AdapterTerminal::Fail(detail) => StepOutcome::Fail(detail),
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.operation.cancel();
    }
}

pub struct StepIo<const PORTS: usize> {
    inputs: [Option<ValueRef>; PORTS],
    input_closed: [bool; PORTS],
    output_maximum_bytes: [Option<u32>; PORTS],
    consumed: [bool; PORTS],
    retained_inputs: [bool; PORTS],
    consumed_closed: [bool; PORTS],
    outputs: [Option<ValueRef>; PORTS],
    discards: [Option<ValueRef>; PORTS],
    host_completion: Option<(RequestId, HostOperationOutcome)>,
    consumed_host_completion: bool,
    host_request: Option<(RequestId, HostOperationId, BoundedValueRef)>,
    maximum_work: u16,
    work: u16,
    fault: Option<SchedulerError>,
}

#[derive(Clone, Copy)]
struct StagedStep<const PORTS: usize> {
    consumed: [bool; PORTS],
    retained_inputs: [bool; PORTS],
    consumed_closed: [bool; PORTS],
    outputs: [Option<ValueRef>; PORTS],
    discards: [Option<ValueRef>; PORTS],
    consumed_host_completion: bool,
    host_request: Option<(RequestId, HostOperationId, BoundedValueRef)>,
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
        self.consume_input(port, false)
    }

    pub fn take_input(&mut self, port: PortId) -> Result<ValueRef, SchedulerError> {
        self.consume_input(port, true)
    }

    pub fn consume_closed(&mut self, port: PortId) -> Result<(), SchedulerError> {
        self.charge_work(1)?;
        let index = usize::from(port.0);
        if !self.input_closed.get(index).copied().unwrap_or(false)
            || self.inputs.get(index).copied().flatten().is_some()
            || self.consumed_closed.get(index).copied().unwrap_or(true)
        {
            return self.fail(SchedulerError::InvalidPortAccess);
        }
        self.consumed_closed[index] = true;
        Ok(())
    }

    fn consume_input(
        &mut self,
        port: PortId,
        retain_for_operation: bool,
    ) -> Result<ValueRef, SchedulerError> {
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
        self.retained_inputs[index] = retain_for_operation;
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

    pub fn discard(&mut self, value: ValueRef) -> Result<(), SchedulerError> {
        self.charge_work(1)?;
        if self
            .discards
            .iter()
            .flatten()
            .any(|discard| *discard == value)
        {
            return self.fail(SchedulerError::InvalidPortAccess);
        }
        let slot = self
            .discards
            .iter_mut()
            .find(|discard| discard.is_none())
            .ok_or(SchedulerError::InvalidPortAccess)?;
        *slot = Some(value);
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
            || self.discards.iter().any(Option::is_some)
            || self.consumed_host_completion
            || self.host_request.is_some()
            || self.consumed_closed.iter().any(|value| *value)
    }

    fn staged_step(&self) -> StagedStep<PORTS> {
        StagedStep {
            consumed: self.consumed,
            retained_inputs: self.retained_inputs,
            consumed_closed: self.consumed_closed,
            outputs: self.outputs,
            discards: self.discards,
            consumed_host_completion: self.consumed_host_completion,
            host_request: self.host_request,
        }
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
    InvalidActiveCapacity,
    InvalidPortAccess,
    InvalidHostOperationAccess,
    OutputBlocked,
    QueueCapacityExceeded,
    QueueByteCapacityExceeded,
    StepWorkExceeded,
    FalseProgress,
    DecisionLimitExceeded,
    OperationFailed(u16),
    OperationProtocolViolation,
    HostOperationCapacityExceeded,
    HostOperationRequestDuplicate,
    HostOperationCompletionRejected,
    HostOperationOutputExceeded,
    InvalidRemoteCordAccess,
    RemoteSequenceRejected,
    RemoteDeliveryRejected,
    ValueOwnershipViolation,
    Cancelled,
    Storage(StorageError),
    Sign(SignError),
    Routing(ProtocolError),
}

impl From<StorageError> for SchedulerError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

impl From<SignError> for SchedulerError {
    fn from(value: SignError) -> Self {
        Self::Sign(value)
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
    E: SignSink,
{
    node_specs: [NodeSpec<PORTS>; NODES],
    cord_specs: [CordSpec; CORDS],
    active_nodes: usize,
    active_cords: usize,
    routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
    host_bindings: Option<FixedHostOperationBindings<HOST_BINDING_SLOTS>>,
    pending_host_operations: [Option<PendingHostOperation>; PENDING_REQUESTS],
    drivers: [D; NODES],
    values: S,
    signs: E,
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
    E: SignSink,
{
    pub fn new(
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        drivers: [D; NODES],
        values: S,
        signs: E,
    ) -> Result<Self, SchedulerError> {
        if NODES == 0 || CORDS == 0 {
            return Err(SchedulerError::InvalidPlan);
        }
        Self::new_with_active_counts(
            NODES, CORDS, node_specs, cord_specs, routes, drivers, values, signs,
        )
    }

    /// Installs an admitted topology prefix inside the compile-time capacity.
    /// Slots outside the active counts remain inert for the scheduler lifetime.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_active_counts(
        active_nodes: usize,
        active_cords: usize,
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        drivers: [D; NODES],
        values: S,
        signs: E,
    ) -> Result<Self, SchedulerError> {
        validate_active_capacity(active_nodes, NODES, active_cords, CORDS)?;
        if PORTS == 0 || QUEUE_SLOTS == 0 || !routes.is_sealed() {
            return Err(SchedulerError::InvalidPlan);
        }
        routes.validate_active_prefix(active_nodes, active_cords)?;
        validate_plan::<NODES, CORDS, PORTS, QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS>(
            active_nodes,
            active_cords,
            &node_specs,
            &cord_specs,
            &routes,
        )?;
        Ok(Self {
            node_specs,
            cord_specs,
            active_nodes,
            active_cords,
            routes,
            host_bindings: None,
            pending_host_operations: [None; PENDING_REQUESTS],
            drivers,
            values,
            signs,
            cords: [CordState::EMPTY; CORDS],
            queue_slots: [None; QUEUE_SLOTS],
            ready: core::array::from_fn(|node| node < active_nodes),
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
        signs: E,
    ) -> Result<Self, SchedulerError> {
        if NODES == 0 || CORDS == 0 {
            return Err(SchedulerError::InvalidPlan);
        }
        Self::new_with_active_counts_and_host_operations(
            NODES,
            CORDS,
            node_specs,
            cord_specs,
            routes,
            host_bindings,
            drivers,
            values,
            signs,
        )
    }

    /// Installs an admitted topology prefix and its sealed host-operation table
    /// inside the compile-time capacities.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_active_counts_and_host_operations(
        active_nodes: usize,
        active_cords: usize,
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        host_bindings: FixedHostOperationBindings<HOST_BINDING_SLOTS>,
        drivers: [D; NODES],
        values: S,
        signs: E,
    ) -> Result<Self, SchedulerError> {
        validate_active_capacity(active_nodes, NODES, active_cords, CORDS)?;
        if PENDING_REQUESTS == 0 || !host_bindings.is_sealed() {
            return Err(SchedulerError::InvalidPlan);
        }
        host_bindings.validate_active_nodes(active_nodes)?;
        let mut scheduler = Self::new_with_active_counts(
            active_nodes,
            active_cords,
            node_specs,
            cord_specs,
            routes,
            drivers,
            values,
            signs,
        )?;
        scheduler.host_bindings = Some(host_bindings);
        Ok(scheduler)
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        if self.cancelled {
            return Ok(SchedulerStatus::Cancelled);
        }
        let Some(node) = self.next_ready() else {
            return if self.completed[..self.active_nodes]
                .iter()
                .all(|value| *value)
                && self.cords[..self.active_cords]
                    .iter()
                    .all(|cord| cord.len == 0)
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
        self.signs
            .record(NodeId(as_u16(node)?), None, None, KernelEventKind::Decision)?;

        let mut io = self.context(node)?;
        let outcome = self.drivers[node].step(&mut io);
        if let Some(fault) = io.fault {
            return Err(fault);
        }
        self.apply_step(node, outcome, io)?;
        if self.completed[..self.active_nodes]
            .iter()
            .all(|value| *value)
            && self.cords[..self.active_cords]
                .iter()
                .all(|cord| cord.len == 0)
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

    pub fn signs(&self) -> &E {
        &self.signs
    }

    pub fn cord_usage(&self, cord: CordId) -> Result<(u16, u32), SchedulerError> {
        if usize::from(cord.0) >= self.active_cords {
            return Err(SchedulerError::InvalidPlan);
        }
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
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
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
            self.ensure_sign_capacity(1)?;
            self.cords[cord_index].offered_remote_sequence = Some(sequence);
            self.signs.record(
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
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
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
        self.ensure_sign_capacity(1)?;
        self.cords[cord_index].remote_accepted = true;
        self.signs.record(
            source_node,
            Some(source_port),
            None,
            KernelEventKind::RemoteValueAccepted,
        )?;
        Ok(())
    }

    /// Releases the source value only after the line reports the exact
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
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
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
        self.ensure_sign_capacity(1)?;
        let value = self.pop(cord_index)?;
        self.values.release(value)?;
        let state = &mut self.cords[cord_index];
        state.next_remote_sequence = next_sequence;
        state.offered_remote_sequence = None;
        state.remote_accepted = false;
        self.ready[usize::from(source_node.0)] = true;
        self.signs.record(
            source_node,
            Some(source_port),
            None,
            KernelEventKind::RemoteValueDelivered,
        )?;
        Ok(())
    }

    /// Admits bytes through a remote ingress cord into the kernel-owned value
    /// store and queue. `Full` performs no allocation or sequence advance, so
    /// the line must retry the same sequence.
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
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
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
        self.ensure_sign_capacity(1)?;
        let value = self.values.store(bytes)?;
        if let Err(error) = self.push(cord_index, value) {
            self.values.release(value)?;
            return Err(error);
        }
        self.cords[cord_index].next_remote_sequence = next_sequence;
        self.ready[usize::from(sink_node.0)] = true;
        self.signs.record(
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
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
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
        self.ensure_sign_capacity(1)?;
        self.cords[cord_index].producer_closed = true;
        self.ready[usize::from(sink_node.0)] = true;
        self.signs.record(
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
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
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
        if usize::from(node.0) >= self.active_nodes {
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
        self.ensure_sign_capacity(1)?;
        if outcome.output.map(|output| output.value) != Some(pending.request.input.value) {
            self.values.release(pending.request.input.value)?;
        }
        self.pending_host_operations[slot]
            .as_mut()
            .ok_or(SchedulerError::HostOperationCompletionRejected)?
            .completion = Some(outcome);
        self.ready[usize::from(pending.request.node.0)] = true;
        self.signs.record(
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
        self.ensure_sign_capacity(2)?;
        self.signs.record(
            NodeId(0),
            None,
            None,
            KernelEventKind::CancellationRequested,
        )?;
        for (node, driver) in self.drivers[..self.active_nodes].iter_mut().enumerate() {
            if !self.completed[node] {
                driver.cancel();
            }
        }
        self.values.clear();
        self.pending_host_operations.fill(None);
        self.queue_slots.fill(None);
        for cord in &mut self.cords[..self.active_cords] {
            cord.head = 0;
            cord.len = 0;
            cord.queued_bytes = 0;
            cord.producer_closed = true;
            cord.offered_remote_sequence = None;
            cord.remote_accepted = false;
        }
        self.ready.fill(false);
        self.cancelled = true;
        self.signs
            .record(NodeId(0), None, None, KernelEventKind::RunCancelled)?;
        Ok(())
    }

    fn next_ready(&mut self) -> Option<usize> {
        for offset in 0..self.active_nodes {
            let node = (self.cursor + offset) % self.active_nodes;
            let waiting_for_host_completion =
                self.pending_host_operations
                    .iter()
                    .flatten()
                    .any(|pending| {
                        usize::from(pending.request.node.0) == node && pending.completion.is_none()
                    });
            if self.ready[node] && !self.completed[node] && !waiting_for_host_completion {
                self.cursor = (node + 1) % self.active_nodes;
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
            let mut sign_records =
                self.commit_event_count(node, &io.consumed, &io.consumed_closed, &io.outputs)?;
            if io.host_request.is_some() {
                sign_records = sign_records
                    .checked_add(1)
                    .ok_or(SchedulerError::InvalidPlan)?;
            }
            if matches!(outcome, StepOutcome::Complete) {
                if io.host_request.is_some() {
                    return Err(SchedulerError::InvalidHostOperationAccess);
                }
                sign_records = sign_records
                    .checked_add(self.output_route_count(node)?)
                    .and_then(|value| value.checked_add(1))
                    .ok_or(SchedulerError::InvalidPlan)?;
            }
            self.ensure_sign_capacity(sign_records)?;
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
                self.signs.record(
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
                self.signs.record(
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
            self.signs.record(
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
            self.signs.record(
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
                self.signs.record(
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

    fn ensure_sign_capacity(&self, additional: usize) -> Result<(), SchedulerError> {
        let additional_items =
            u16::try_from(additional).map_err(|_| SchedulerError::InvalidPlan)?;
        if self
            .signs
            .len()
            .checked_add(additional_items)
            .filter(|len| *len <= self.signs.item_capacity())
            .is_none()
        {
            return Err(SchedulerError::Sign(SignError::ItemCapacityExceeded));
        }
        let charge = u32::try_from(core::mem::size_of::<crate::KernelEvent>())
            .map_err(|_| SchedulerError::InvalidPlan)?
            .checked_mul(u32::from(additional_items))
            .ok_or(SchedulerError::InvalidPlan)?;
        if self
            .signs
            .used_bytes()
            .checked_add(charge)
            .filter(|bytes| *bytes <= self.signs.byte_capacity())
            .is_none()
        {
            return Err(SchedulerError::Sign(SignError::ByteCapacityExceeded));
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
                    self.signs.record(
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
    active_nodes: usize,
    active_cords: usize,
    nodes: &[NodeSpec<PORTS>; NODES],
    cords: &[CordSpec; CORDS],
    routes: &FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
) -> Result<(), SchedulerError> {
    for (node_index, node) in nodes[..active_nodes].iter().enumerate() {
        if node.maximum_step_work == 0 {
            return Err(SchedulerError::InvalidPlan);
        }
        for (port, cord) in node.input_cords.iter().copied().enumerate() {
            let Some(cord) = cord else {
                continue;
            };
            let spec = cords
                .get(usize::from(cord.0))
                .filter(|_| usize::from(cord.0) < active_cords)
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
                let spec = cords
                    .get(cord)
                    .filter(|_| cord < active_cords)
                    .ok_or(SchedulerError::InvalidPlan)?;
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
    for (cord_index, cord) in cords[..active_cords].iter().copied().enumerate() {
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
                if usize::from(source_node.0) >= active_nodes
                    || usize::from(sink_node.0) >= active_nodes
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
                if usize::from(source_node.0) >= active_nodes || usize::from(source_port.0) >= PORTS
                {
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
                if usize::from(sink_node.0) >= active_nodes || usize::from(sink_port.0) >= PORTS {
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
