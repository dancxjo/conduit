use crate::{
    BoundedValueRef, EvidenceError, HostOperationId, HostOperationOutcome, NodeId, Operation,
    OperationAction, OperationInput, PortId, ProtocolError, RequestId, StorageError, ValueRef,
};

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
pub(super) struct PendingHostOperation {
    pub(super) request: HostOperationRequest,
    pub(super) maximum_output_bytes: u32,
    pub(super) dispatched: bool,
    pub(super) completion: Option<HostOperationOutcome>,
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
    pub(super) inputs: [Option<ValueRef>; PORTS],
    pub(super) input_closed: [bool; PORTS],
    pub(super) output_maximum_bytes: [Option<u32>; PORTS],
    pub(super) consumed: [bool; PORTS],
    pub(super) retained_inputs: [bool; PORTS],
    pub(super) consumed_closed: [bool; PORTS],
    pub(super) outputs: [Option<ValueRef>; PORTS],
    pub(super) discards: [Option<ValueRef>; PORTS],
    pub(super) host_completion: Option<(RequestId, HostOperationOutcome)>,
    pub(super) consumed_host_completion: bool,
    pub(super) host_request: Option<(RequestId, HostOperationId, BoundedValueRef)>,
    pub(super) maximum_work: u16,
    pub(super) work: u16,
    pub(super) fault: Option<SchedulerError>,
}

#[derive(Clone, Copy)]
pub(super) struct StagedStep<const PORTS: usize> {
    pub(super) consumed: [bool; PORTS],
    pub(super) retained_inputs: [bool; PORTS],
    pub(super) consumed_closed: [bool; PORTS],
    pub(super) outputs: [Option<ValueRef>; PORTS],
    pub(super) discards: [Option<ValueRef>; PORTS],
    pub(super) consumed_host_completion: bool,
    pub(super) host_request: Option<(RequestId, HostOperationId, BoundedValueRef)>,
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

    pub(super) fn staged(&self) -> bool {
        self.consumed.iter().any(|value| *value)
            || self.outputs.iter().any(Option::is_some)
            || self.discards.iter().any(Option::is_some)
            || self.consumed_host_completion
            || self.host_request.is_some()
            || self.consumed_closed.iter().any(|value| *value)
    }

    pub(super) fn staged_step(&self) -> StagedStep<PORTS> {
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
