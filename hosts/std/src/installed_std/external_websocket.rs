use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedOperation};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId, ValueRef, ValueStorage,
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

impl ExternalWebSocketListenerOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        self.request_accept()
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => self.request(
                Pending::Send,
                value,
                conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES,
            ),
            OperationInput::HostOperationCompleted { request, outcome }
                if request == RequestId(self.next_request.saturating_sub(1)) =>
            {
                let Some(pending) = self.pending.take() else {
                    return InstalledOperation::fail(20);
                };
                match pending {
                    Pending::Accept(peer)
                        if outcome.disposition == HostOperationDisposition::Completed
                            && outcome.failure.is_none() =>
                    {
                        if outcome.output.is_some() {
                            return InstalledOperation::fail(21);
                        }
                        self.connected[peer] = true;
                        self.accepted += 1;
                        if self.accepted < self.connected.len() {
                            self.request_accept()
                        } else {
                            self.request_receive()
                        }
                    }
                    Pending::Receive(_peer)
                        if outcome.disposition == HostOperationDisposition::Completed
                            && outcome.failure.is_none() =>
                    {
                        let Some(output) = outcome.output else {
                            return InstalledOperation::fail(22);
                        };
                        self.received = self.received.saturating_add(1);
                        self.after_emit = AfterEmit::AwaitSend;
                        OperationAction::Emit {
                            port: PortId(1),
                            value: output.value,
                        }
                    }
                    Pending::Receive(peer)
                        if outcome.disposition == HostOperationDisposition::Cancelled
                            && outcome.failure.is_none() =>
                    {
                        self.connected[peer] = false;
                        if self.connected.iter().any(|connected| *connected) {
                            let Some(output) = outcome.output else {
                                return InstalledOperation::fail(25);
                            };
                            self.request_receive_with(output.value)
                        } else {
                            OperationAction::Complete
                        }
                    }
                    Pending::Send
                        if outcome.disposition == HostOperationDisposition::Completed
                            && outcome.failure.is_none() =>
                    {
                        let Some(output) = outcome.output else {
                            return InstalledOperation::fail(26);
                        };
                        self.request_receive_with(output.value)
                    }
                    _ => InstalledOperation::fail(23),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(24),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        match self.after_emit {
            AfterEmit::AwaitSend => {
                if self.received >= conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_HISTORY_ITEMS {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }

    fn request_accept(&mut self) -> OperationAction {
        let peer = self.accepted;
        self.request(Pending::Accept(peer), self.accept_commands[peer], 64)
    }

    fn request_receive(&mut self) -> OperationAction {
        let Some(peer) = (0..self.connected.len())
            .map(|offset| (self.receive_cursor + offset) % self.connected.len())
            .find(|peer| self.connected[*peer])
        else {
            return OperationAction::Complete;
        };
        self.receive_cursor = (peer + 1) % self.connected.len();
        let Some(value) = self.initial_receive_command.take() else {
            return OperationAction::Complete;
        };
        self.request(Pending::Receive(peer), value, 1)
    }

    fn request_receive_with(&mut self, value: ValueRef) -> OperationAction {
        let Some(peer) = (0..self.connected.len())
            .map(|offset| (self.receive_cursor + offset) % self.connected.len())
            .find(|peer| self.connected[*peer])
        else {
            return OperationAction::Complete;
        };
        self.receive_cursor = (peer + 1) % self.connected.len();
        self.request(
            Pending::Receive(peer),
            value,
            conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES,
        )
    }

    fn request(&mut self, pending: Pending, value: ValueRef, maximum: u32) -> OperationAction {
        let request = RequestId(self.next_request);
        self.next_request = self.next_request.saturating_add(1);
        self.pending = Some(pending);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(match pending {
                Pending::Accept(_) => 0,
                Pending::Receive(_) => 1,
                Pending::Send => 2,
            }),
            input: BoundedValueRef::new(value, maximum).expect("prepared host input is bounded"),
        }
    }
}

fn budget(placement: &PlannedOperation) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_ITEMS,
        value_bytes: conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_BYTES,
        host_requests: 2
            + usize::from(conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_HISTORY_ITEMS) * 2
            + 2,
        evidence_items: 512,
        maximum_value_bytes: conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES,
    })
}

fn prepare(
    placement: &PlannedOperation,
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

fn validate(placement: &PlannedOperation) -> Result<(), String> {
    let offer = conduit_net::std_external_websocket_family().capability;
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.execution_profile_id
        || placement.implementation_id != offer.implementation_id
        || placement.artifact_id != offer.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
    {
        return Err("external WebSocket listener placement differs from its installation".into());
    }
    Ok(())
}
