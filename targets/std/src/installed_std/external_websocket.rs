use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};

pub(super) static EXTERNAL_WEBSOCKET_LISTENER_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: "std/native-external-websocket-listener@1",
    budget,
    prepare,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Pending {
    Accept(usize),
    Receive(usize),
    Send,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AfterEmit {
    AwaitSend,
}

pub(super) struct ExternalWebSocketListenerOperation {
    accept_commands: [ValueRef; 2],
    initial_receive_command: Option<ValueRef>,
    connected: [bool; 2],
    accepted: usize,
    receive_cursor: usize,
    received: u16,
    next_request: u32,
    pending: Option<Pending>,
    after_emit: AfterEmit,
}

impl<const PORTS: usize> StepOperation<PORTS> for ExternalWebSocketListenerOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if request != RequestId(self.next_request.saturating_sub(1)) {
                return step_fail(20);
            }
            let Some(pending) = self.pending else {
                return step_fail(20);
            };
            match pending {
                Pending::Accept(peer)
                    if outcome.disposition == HostCallDisposition::Completed
                        && outcome.failure.is_none()
                        && outcome.output.is_none() =>
                {
                    io.consume_host_completion()
                        .expect("observed external WebSocket accept completion");
                    self.pending = None;
                    self.connected[peer] = true;
                    self.accepted += 1;
                    if self.accepted < self.connected.len() {
                        return self.request_accept_step(io);
                    }
                    return self.request_initial_receive_step(io);
                }
                Pending::Receive(_peer)
                    if outcome.disposition == HostCallDisposition::Completed
                        && outcome.failure.is_none() =>
                {
                    let Some(output) = outcome.output else {
                        return step_fail(22);
                    };
                    if !io.output_ready(PortId(1)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed external WebSocket receive completion");
                    io.send(PortId(1), output.value)
                        .expect("ready external WebSocket receive output");
                    self.pending = None;
                    self.received = self.received.saturating_add(1);
                    self.after_emit = AfterEmit::AwaitSend;
                    return if self.received >= conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_HISTORY_ITEMS
                    {
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Progress
                    };
                }
                Pending::Receive(peer)
                    if outcome.disposition == HostCallDisposition::Cancelled
                        && outcome.failure.is_none() =>
                {
                    let Some(output) = outcome.output else {
                        return step_fail(25);
                    };
                    io.consume_host_completion()
                        .expect("observed external WebSocket peer closure");
                    self.pending = None;
                    self.connected[peer] = false;
                    if self.connected.iter().any(|connected| *connected) {
                        return self.request_receive_step(io, output.value);
                    }
                    return StepOutcome::Complete;
                }
                Pending::Send
                    if outcome.disposition == HostCallDisposition::Completed
                        && outcome.failure.is_none() =>
                {
                    let Some(output) = outcome.output else {
                        return step_fail(26);
                    };
                    io.consume_host_completion()
                        .expect("observed external WebSocket send completion");
                    self.pending = None;
                    return self.request_receive_step(io, output.value);
                }
                _ => return step_fail(23),
            }
        }

        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return step_fail(24);
            }
            let Ok(input) = BoundedValueRef::new(
                value,
                conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES,
            ) else {
                return step_fail(24);
            };
            let Some((request, next)) = self.next_request_id() else {
                return step_failure(FailureCode::StorageExhausted, 27);
            };
            io.consume(PortId(0))
                .expect("present external WebSocket send input");
            io.request_host_call(request, HostCallId(2), input)
                .expect("external WebSocket send Host Call");
            self.next_request = next;
            self.pending = Some(Pending::Send);
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed external WebSocket send closure");
            return StepOutcome::Complete;
        }

        if self.pending.is_none() && self.accepted == 0 {
            return self.request_accept_step(io);
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl ExternalWebSocketListenerOperation {
    fn next_request_id(&self) -> Option<(RequestId, u32)> {
        self.next_request
            .checked_add(1)
            .map(|next| (RequestId(self.next_request), next))
    }

    fn request_accept_step<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        let peer = self.accepted;
        let Some(value) = self.accept_commands.get(peer).copied() else {
            return step_fail(21);
        };
        self.request_step(io, Pending::Accept(peer), value, 64)
    }

    fn request_initial_receive_step<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
    ) -> StepOutcome {
        let Some(value) = self.initial_receive_command.take() else {
            return StepOutcome::Complete;
        };
        self.request_receive_step(io, value)
    }

    fn request_receive_step<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        value: ValueRef,
    ) -> StepOutcome {
        let Some(peer) = (0..self.connected.len())
            .map(|offset| (self.receive_cursor + offset) % self.connected.len())
            .find(|peer| self.connected[*peer])
        else {
            return StepOutcome::Complete;
        };
        self.receive_cursor = (peer + 1) % self.connected.len();
        self.request_step(
            io,
            Pending::Receive(peer),
            value,
            conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES,
        )
    }

    fn request_step<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        pending: Pending,
        value: ValueRef,
        maximum: u32,
    ) -> StepOutcome {
        let Some((request, next)) = self.next_request_id() else {
            return step_failure(FailureCode::StorageExhausted, 27);
        };
        let Ok(input) = BoundedValueRef::new(value, maximum) else {
            return step_fail(24);
        };
        io.request_host_call(
            request,
            HostCallId(match pending {
                Pending::Accept(_) => 0,
                Pending::Receive(_) => 1,
                Pending::Send => 2,
            }),
            input,
        )
        .expect("external WebSocket Host Call");
        self.next_request = next;
        self.pending = Some(pending);
        StepOutcome::Progress
    }
}

const fn step_failure(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

const fn step_fail(detail: u16) -> StepOutcome {
    step_failure(FailureCode::InvalidLifecycle, detail)
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_ITEMS,
        value_bytes: conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_BYTES,
        host_requests: 2
            + usize::from(conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_HISTORY_ITEMS) * 2
            + 2,
        sign_items: 512,
        maximum_value_bytes: conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let bind = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("bind", ConfigurationValue::Text(value)) => Some(value.as_bytes()),
            _ => None,
        })
        .ok_or_else(|| "external WebSocket listener has no bind address".to_string())?;
    let accept_commands = [store(values, bind)?, store(values, bind)?];
    let initial_receive_command = store(values, &[0])?;
    Ok(InstalledOperation::ExternalWebSocketListener(
        ExternalWebSocketListenerOperation {
            accept_commands,
            initial_receive_command: Some(initial_receive_command),
            connected: [false; 2],
            accepted: 0,
            receive_cursor: 0,
            received: 0,
            next_request: 0,
            pending: None,
            after_emit: AfterEmit::AwaitSend,
        },
    ))
}

fn store(values: &mut conduit_kernel::HostedValueStore, bytes: &[u8]) -> Result<ValueRef, String> {
    values
        .store(bytes)
        .map_err(|error| format!("store external WebSocket command: {error:?}"))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_net::std_external_websocket_family().capability;
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
    {
        return Err("external WebSocket listener placement differs from its installation".into());
    }
    Ok(())
}
