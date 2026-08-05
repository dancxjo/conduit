use crate::{CordEndpoint, CordId, NodeId, PortId, RemoteEndpointId, ValueRef};

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
