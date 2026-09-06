//! Fixed-capacity operation adaptation and transactional ownership hooks.
//! Scheduling, queue admission and commit remain in the parent scheduler.
use super::{CanonicalValue, SchedulerError, StepInputBytes, StepIo, StepOperation, StepOutcome};
use crate::{
    BoundedValueRef, HostOperationId, HostOperationOutcome, Operation, OperationAction,
    OperationInput, PortId, RequestId, ValueRef,
};

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
    canonical_output: Option<(PortId, CanonicalValue)>,
    host_request: Option<(RequestId, HostOperationId, BoundedValueRef)>,
    host_cancellation: Option<RequestId>,
    retain_resumed_value: bool,
    released_values: [Option<ValueRef>; PORTS],
    terminal: AdapterTerminal,
}

impl<const PORTS: usize> AdapterTransaction<PORTS> {
    fn is_empty_continue(&self) -> bool {
        self.outputs.iter().all(Option::is_none)
            && self.canonical_output.is_none()
            && self.host_request.is_none()
            && self.host_cancellation.is_none()
            && !self.retain_resumed_value
            && self.released_values.iter().all(Option::is_none)
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
        let mut transaction = driver.collect(AdapterEvent::None, first)?;
        driver.collect_released_values(&mut transaction)?;
        driver.collect_host_cancellation(&mut transaction)?;
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
            canonical_output: None,
            host_request: None,
            host_cancellation: None,
            retain_resumed_value: false,
            released_values: [None; PORTS],
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
                OperationAction::EmitCanonical { port, value } => {
                    let output = transaction
                        .outputs
                        .get(usize::from(port.0))
                        .ok_or(SchedulerError::InvalidPortAccess)?;
                    if output.is_some() || transaction.canonical_output.is_some() {
                        return Err(SchedulerError::OperationProtocolViolation);
                    }
                    transaction.canonical_output = Some((port, value));
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

    fn collect_released_values(
        &mut self,
        transaction: &mut AdapterTransaction<PORTS>,
    ) -> Result<(), SchedulerError> {
        for released in &mut transaction.released_values {
            let Some(value) = self.operation.take_released_value() else {
                return Ok(());
            };
            *released = Some(value);
        }
        if self.operation.take_released_value().is_some() {
            return Err(SchedulerError::OperationProtocolViolation);
        }
        Ok(())
    }

    fn collect_host_cancellation(
        &mut self,
        transaction: &mut AdapterTransaction<PORTS>,
    ) -> Result<(), SchedulerError> {
        transaction.host_cancellation = self.operation.take_host_operation_cancellation();
        if self.operation.take_host_operation_cancellation().is_some() {
            return Err(SchedulerError::OperationProtocolViolation);
        }
        Ok(())
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
    fn step_committed(&mut self) {
        self.operation.step_committed();
    }
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.protocol_failed {
            return StepOutcome::Fail(u16::MAX);
        }
        if self.pending.is_none() {
            let Some(event) = self.next_event(io) else {
                return StepOutcome::Await;
            };
            let action = match event {
                AdapterEvent::Value { port, value } => {
                    let Some(canonical) = input_bytes.input(port) else {
                        self.protocol_failed = true;
                        return StepOutcome::Fail(u16::MAX);
                    };
                    self.operation.resume_value(port, value, canonical)
                }
                AdapterEvent::HostCompleted { request, outcome } => self
                    .operation
                    .resume_host_operation(request, outcome, input_bytes.host_output()),
                _ => {
                    let Some(input) = event.operation_input() else {
                        return StepOutcome::Fail(u16::MAX);
                    };
                    self.operation.resume(input)
                }
            };
            match self.collect(event, action) {
                Ok(mut transaction) => {
                    transaction.retain_resumed_value = matches!(event, AdapterEvent::Value { .. })
                        && self.operation.retains_resumed_value();
                    if self.collect_released_values(&mut transaction).is_err() {
                        self.protocol_failed = true;
                        return StepOutcome::Fail(u16::MAX);
                    }
                    if self.collect_host_cancellation(&mut transaction).is_err() {
                        self.protocol_failed = true;
                        return StepOutcome::Fail(u16::MAX);
                    }
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
        if let Some((port, _)) = transaction.canonical_output {
            if !io.output_ready(port) {
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
        for value in transaction.released_values.into_iter().flatten() {
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
        if let Some((port, value)) = transaction.canonical_output {
            if io.send_canonical(port, value).is_err() {
                return StepOutcome::Fail(u16::MAX);
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
        if let Some(request) = transaction.host_cancellation {
            if io.cancel_host_operation(request).is_err() {
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

    fn accepts_input_while_host_operation_pending(&self) -> bool {
        self.operation.accepts_input_while_host_operation_pending()
    }
}

impl<O, const PORTS: usize> OperationDriver<O, PORTS> {
    /// Consumes an owned driver. Active schedulers expose only borrowed drivers.
    pub fn into_operation(self) -> O {
        self.operation
    }
}
#[cfg(test)]
mod tests {
    use super::AdapterTransaction;
    #[test]
    fn derived_output_staging_is_bounded_independently_of_port_capacity() {
        assert!(core::mem::size_of::<AdapterTransaction<32>>() < 1_024);
    }
}
