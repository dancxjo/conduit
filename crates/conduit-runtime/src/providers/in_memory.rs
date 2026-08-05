use conduit_core::{
    ConnectionEnvelope, ConnectionOutcome, KindId, PlanId, PlannedConnection, PROTOCOL_VERSION,
};
use std::collections::VecDeque;

#[derive(Debug)]
pub struct InMemoryConnectionProvider {
    plan_id: PlanId,
    connection_id: conduit_core::ConnectionId,
    value_kind: KindId,
    item_capacity: usize,
    byte_capacity: u32,
    queued_bytes: u32,
    next_sequence: u64,
    terminal: bool,
    queue: VecDeque<ConnectionEnvelope>,
}

impl InMemoryConnectionProvider {
    pub fn new(plan_id: PlanId, connection: &PlannedConnection) -> Self {
        Self {
            plan_id,
            connection_id: connection.connection_id.clone(),
            value_kind: connection.value_kind.clone(),
            item_capacity: connection.item_capacity as usize,
            byte_capacity: connection.byte_capacity,
            queued_bytes: 0,
            next_sequence: 0,
            terminal: false,
            queue: VecDeque::new(),
        }
    }

    pub fn status(&self) -> ConnectionOutcome {
        if self.terminal {
            ConnectionOutcome::Terminal
        } else if self.queue.len() >= self.item_capacity || self.queued_bytes >= self.byte_capacity
        {
            ConnectionOutcome::Full
        } else {
            ConnectionOutcome::Ready
        }
    }

    pub fn accept(&mut self, envelope: ConnectionEnvelope) -> ConnectionOutcome {
        if self.terminal {
            return ConnectionOutcome::Terminal;
        }
        if envelope.protocol_version != PROTOCOL_VERSION
            || envelope.plan_id != self.plan_id
            || envelope.connection_id != self.connection_id
            || envelope.value_kind != self.value_kind
            || envelope.sequence != self.next_sequence
            || envelope.encoded_len() > self.byte_capacity
        {
            return ConnectionOutcome::Malformed;
        }
        if self.queue.len() >= self.item_capacity
            || self.queued_bytes + envelope.encoded_len() > self.byte_capacity
        {
            return ConnectionOutcome::Full;
        }
        self.queued_bytes += envelope.encoded_len();
        self.next_sequence += 1;
        self.queue.push_back(envelope);
        ConnectionOutcome::Accepted
    }

    pub fn deliver(&mut self) -> Option<(ConnectionOutcome, ConnectionEnvelope)> {
        if self.terminal {
            return None;
        }
        let envelope = self.queue.pop_front()?;
        self.queued_bytes -= envelope.encoded_len();
        Some((ConnectionOutcome::Delivered, envelope))
    }

    pub fn disconnect(&mut self) -> ConnectionOutcome {
        self.terminal = true;
        self.queue.clear();
        self.queued_bytes = 0;
        ConnectionOutcome::Disconnected
    }

    pub fn queued_items(&self) -> usize {
        self.queue.len()
    }

    pub fn queued_bytes(&self) -> u32 {
        self.queued_bytes
    }
}
