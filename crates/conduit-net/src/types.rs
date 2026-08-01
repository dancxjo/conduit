//! Host-neutral standing-network value contracts and bounded reference state.
//!
//! These values deliberately preserve network layer boundaries. A link
//! observation is not a frame, a frame is not a packet, a datagram is not a
//! byte stream, and none of those values establishes identity or authority.

use conduit_core::{Id, SemanticHash, TypeContractRef};

use crate::{Ipv4Address, MAXIMUM_PACKET_BYTES, NetworkReason};

pub const MAXIMUM_FRAME_BYTES: usize = 1_518;
pub const MAXIMUM_DATAGRAM_BYTES: usize = 1_472;
pub const MAXIMUM_STREAM_CHUNK_BYTES: usize = 1_024;
pub const MAXIMUM_ROUTES: usize = 16;
pub const MAXIMUM_SESSIONS: usize = 8;

pub const LINK_OBSERVATION_DESCRIPTOR: &str = "conduit.net/link-observation|0|interface-generation,kind,carrier,mtu,address-readiness,availability,observed-at,valid-until|finite";
pub const FRAME_DESCRIPTOR: &str =
    "conduit.net/frame|0|interface,direction,protocol,length,observed-at,payload|bounded";
pub const PACKET_DESCRIPTOR: &str = "conduit.net/packet|0|sequence,family,source,destination,hop-limit,fragmentation,egress-interface,disposition,payload|bounded";
pub const DATAGRAM_DESCRIPTOR: &str = "conduit.net/datagram|0|session,sequence,family,source,source-port,destination,destination-port,delivery,payload|message-preserving-bounded";
pub const BYTE_STREAM_DESCRIPTOR: &str = "conduit.net/byte-stream|0|session,offset,bytes,eof,read-half-close,write-half-close,pressure|ordered-bounded";
pub const SESSION_DESCRIPTOR: &str = "conduit.net/session|0|identity,generation,family,protocol,local,local-port,peer,peer-port,lifecycle,authenticated,expires-at|finite";
pub const CONTROL_EVENT_DESCRIPTOR: &str =
    "conduit.net/control-event|0|identity,generation,kind,outcome,tick|discrete";
pub const RETAINED_NETWORK_STATE_DESCRIPTOR: &str = "conduit.net/retained-state|0|table,generation,items,bytes,observed-at,expires-at,policy|finite";
pub const ADDRESS_STATE_DESCRIPTOR: &str = "conduit.net/address-state|0|interface,generation,family,address,prefix,readiness,valid-until|finite";
pub const DHCP_LEASE_DESCRIPTOR: &str =
    "conduit.net/dhcp-lease|0|client,family,address,generation,phase,expires-at,server|finite";
pub const NEIGHBOR_STATE_DESCRIPTOR: &str = "conduit.net/neighbor-state|0|interface,family,address,link-address,generation,phase,expires-at|finite";
pub const ROUTE_STATE_DESCRIPTOR: &str = "conduit.net/route-state|0|generation,family,prefix,prefix-length,next-hop,interface,mtu,policy|finite";
pub const SERVICE_REGISTRATION_DESCRIPTOR: &str = "conduit.net/service-registration|0|name,family,address,port,protocol,generation,expires-at|finite";
pub const REACHABILITY_OBSERVATION_DESCRIPTOR: &str = "conduit.net/reachability-observation|0|family,target,scope,outcome,latency,observed-at,valid-until|finite-non-auth";

pub const LINK_OBSERVATION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/link-observation"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x0e, 0xe5, 0xfc, 0x0b, 0x87, 0x3d, 0x70, 0xa2, 0xe5, 0x38, 0x16, 0xea, 0x5b, 0x2c, 0xb8,
        0xa3, 0x63, 0x3c, 0x06, 0x10, 0x4c, 0xf3, 0xd3, 0xee, 0x4b, 0xc1, 0x5a, 0x3c, 0xf1, 0x4a,
        0xca, 0x02,
    ]),
};
pub const FRAME_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/frame"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x14, 0xa0, 0x2e, 0x04, 0x31, 0xc1, 0x53, 0x73, 0xf1, 0xbb, 0x62, 0x1a, 0x81, 0x3a, 0xdc,
        0xf2, 0xbe, 0x90, 0x99, 0xc0, 0x70, 0xa5, 0x4a, 0x5c, 0xed, 0x1a, 0x4c, 0xba, 0xe2, 0x57,
        0xf3, 0x4b,
    ]),
};
pub const PACKET_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/packet"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x4d, 0x20, 0x7e, 0x68, 0x27, 0x7c, 0x90, 0x48, 0xa2, 0x02, 0x92, 0xba, 0xb7, 0xce, 0xfb,
        0x77, 0xb7, 0xdf, 0x6b, 0xda, 0xfd, 0x46, 0x65, 0x95, 0xd4, 0x37, 0x2f, 0xb4, 0xc5, 0x00,
        0x10, 0xbd,
    ]),
};
pub const DATAGRAM_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/datagram"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x84, 0x12, 0x94, 0x20, 0xb6, 0x9e, 0x60, 0xfc, 0x4e, 0xc7, 0x2a, 0xec, 0x00, 0x54, 0xd2,
        0x3c, 0xb7, 0x25, 0x52, 0x9c, 0x0f, 0xc3, 0xa0, 0xdb, 0x9f, 0x25, 0xa5, 0x83, 0xcb, 0x72,
        0x33, 0x78,
    ]),
};
pub const BYTE_STREAM_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/byte-stream"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xdb, 0xae, 0x71, 0xae, 0x4d, 0xca, 0x13, 0x18, 0xb9, 0x51, 0x62, 0x27, 0xe7, 0x11, 0x3d,
        0x03, 0x4b, 0x45, 0xe2, 0xbb, 0xa1, 0x2e, 0x81, 0xcc, 0x58, 0xe9, 0x47, 0x41, 0x37, 0xd7,
        0x26, 0x0f,
    ]),
};
pub const SESSION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/session"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x43, 0x9e, 0x48, 0x2e, 0x79, 0x76, 0x1a, 0x55, 0xc1, 0xb1, 0x89, 0x1e, 0x63, 0x48, 0x50,
        0x6a, 0xc3, 0x24, 0x64, 0xb9, 0x60, 0x66, 0x9e, 0xdc, 0xf5, 0x19, 0x20, 0x39, 0x70, 0xd7,
        0x5e, 0xbf,
    ]),
};
pub const CONTROL_EVENT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/control-event"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xbf, 0xbc, 0x5c, 0x81, 0x42, 0x06, 0x21, 0xd7, 0xc8, 0xa1, 0x73, 0x50, 0xc7, 0x74, 0x4d,
        0xeb, 0x65, 0xb8, 0xda, 0x32, 0x60, 0x28, 0x13, 0x90, 0xce, 0x11, 0x15, 0x5c, 0x96, 0xb3,
        0xa5, 0x94,
    ]),
};
pub const RETAINED_NETWORK_STATE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/retained-state"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x6f, 0x0b, 0x39, 0x25, 0x84, 0xc8, 0x49, 0xd4, 0xf7, 0xa3, 0x14, 0x2d, 0x6f, 0x32, 0xb2,
        0x3a, 0x2a, 0xca, 0x85, 0xe3, 0x94, 0x93, 0x80, 0x77, 0xba, 0x0a, 0x68, 0x06, 0x4e, 0x35,
        0x6c, 0x0e,
    ]),
};
pub const ADDRESS_STATE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/address-state"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xc5, 0xe2, 0xc0, 0xb3, 0x7a, 0x7a, 0x14, 0xca, 0xad, 0x4e, 0x5b, 0x60, 0xeb, 0x82, 0xe7,
        0xe3, 0xdc, 0x87, 0xc3, 0xf4, 0xef, 0x98, 0x9d, 0x09, 0xcc, 0xd2, 0x3b, 0xfa, 0x2a, 0x5b,
        0x11, 0xfe,
    ]),
};
pub const DHCP_LEASE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/dhcp-lease"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf7, 0x82, 0xe5, 0xc0, 0xfc, 0xcc, 0x18, 0x6c, 0x6e, 0x93, 0x4a, 0x39, 0x93, 0x15, 0xc2,
        0xf9, 0x98, 0xae, 0x2b, 0xf0, 0x34, 0xb9, 0xf3, 0xf6, 0xd6, 0x88, 0x30, 0x1e, 0xef, 0x25,
        0xc5, 0xc4,
    ]),
};
pub const NEIGHBOR_STATE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/neighbor-state"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x37, 0x20, 0x7b, 0x03, 0x39, 0xb1, 0x55, 0x21, 0xdb, 0x77, 0x42, 0xf0, 0xf0, 0x0c, 0x02,
        0x53, 0x86, 0x0e, 0x5c, 0x3d, 0x5a, 0x41, 0x7d, 0x91, 0x82, 0x25, 0xce, 0xa5, 0xe7, 0x14,
        0x88, 0x41,
    ]),
};
pub const ROUTE_STATE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/route-state"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x95, 0xb3, 0x2e, 0x94, 0x07, 0x37, 0x88, 0x6b, 0x9d, 0x71, 0x8c, 0x56, 0x2c, 0xae, 0x5b,
        0x35, 0xa2, 0x33, 0x37, 0x57, 0x6f, 0xae, 0xb3, 0x7c, 0xa5, 0x5e, 0x9c, 0xa0, 0x4f, 0x13,
        0x45, 0x20,
    ]),
};
pub const SERVICE_REGISTRATION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/service-registration"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xc2, 0x36, 0x6c, 0xa4, 0x53, 0xdd, 0xf5, 0xef, 0x15, 0x40, 0xc1, 0xf7, 0xec, 0xa5, 0xad,
        0x82, 0x42, 0x96, 0x47, 0x05, 0xba, 0xaa, 0xfd, 0x3a, 0xc6, 0xeb, 0x53, 0x1b, 0xd5, 0x59,
        0x02, 0x4b,
    ]),
};
pub const REACHABILITY_OBSERVATION_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.net/reachability-observation"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xc3, 0xbe, 0x7e, 0x12, 0xd0, 0x14, 0xda, 0x88, 0x6c, 0x0d, 0x5f, 0x09, 0x9f, 0x0b, 0x42,
        0x02, 0xf8, 0x77, 0x54, 0xe4, 0x6e, 0xac, 0x4e, 0xca, 0xa7, 0x03, 0xf9, 0xcd, 0xd4, 0x88,
        0x80, 0x3f,
    ]),
};

pub const NETWORK_VALUE_TYPES: [TypeContractRef<'static>; 14] = [
    LINK_OBSERVATION_TYPE,
    FRAME_TYPE,
    PACKET_TYPE,
    DATAGRAM_TYPE,
    BYTE_STREAM_TYPE,
    SESSION_TYPE,
    CONTROL_EVENT_TYPE,
    RETAINED_NETWORK_STATE_TYPE,
    ADDRESS_STATE_TYPE,
    DHCP_LEASE_TYPE,
    NEIGHBOR_STATE_TYPE,
    ROUTE_STATE_TYPE,
    SERVICE_REGISTRATION_TYPE,
    REACHABILITY_OBSERVATION_TYPE,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkAvailability {
    Unsupported,
    Active,
    Waiting,
    Degraded,
    Draining,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkKind {
    Ethernet,
    WifiStation,
    WifiAccessPoint,
    Usb,
    Loopback,
    Virtual,
    Embedded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkObservation {
    pub interface: u16,
    pub generation: u32,
    pub kind: LinkKind,
    pub carrier: bool,
    pub mtu: u16,
    pub address_ready: bool,
    pub availability: NetworkAvailability,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkDirection {
    Ingress,
    Egress,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkFrame {
    pub interface: u16,
    pub direction: NetworkDirection,
    pub protocol: Option<u16>,
    pub observed_at_tick: u64,
    pub payload: Vec<u8>,
}

impl NetworkFrame {
    pub fn validate(&self) -> Result<(), NetworkReason> {
        if self.interface == 0
            || self.protocol == Some(0)
            || self.payload.is_empty()
            || self.payload.len() > MAXIMUM_FRAME_BYTES
        {
            return Err(NetworkReason::Bounds);
        }
        Ok(())
    }
}

impl LinkObservation {
    pub fn validate(self, now_tick: u64) -> Result<(), NetworkReason> {
        if self.interface == 0 || self.mtu == 0 || self.observed_at_tick > now_tick {
            return Err(NetworkReason::Bounds);
        }
        if now_tick >= self.valid_until_tick {
            return Err(NetworkReason::ObservationStale);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressReadiness {
    Tentative,
    Ready,
    Duplicate,
    Expired,
    Removed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkAddressState {
    pub interface: u16,
    pub generation: u32,
    pub family: AddressFamily,
    pub address: [u8; 16],
    pub prefix_length: u8,
    pub readiness: AddressReadiness,
    pub valid_until_tick: Option<u64>,
}

impl NetworkAddressState {
    pub fn validate(self) -> Result<(), NetworkReason> {
        let maximum_prefix = match self.family {
            AddressFamily::Ipv4 => 32,
            AddressFamily::Ipv6 => 128,
        };
        if self.interface == 0
            || self.generation == 0
            || self.prefix_length > maximum_prefix
            || !address_is_canonical(self.family, &self.address)
        {
            return Err(NetworkReason::Bounds);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeasePhase {
    Offered,
    Bound,
    Renewed,
    Rebinding,
    Released,
    Expired,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkDhcpLease {
    pub client: u64,
    pub family: AddressFamily,
    pub address: [u8; 16],
    pub generation: u32,
    pub phase: LeasePhase,
    pub expires_at_tick: Option<u64>,
    pub server: [u8; 16],
}

impl NetworkDhcpLease {
    pub fn validate(self) -> Result<(), NetworkReason> {
        if self.client == 0
            || self.generation == 0
            || !address_is_canonical(self.family, &self.address)
            || !address_is_canonical(self.family, &self.server)
        {
            return Err(NetworkReason::Bounds);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeighborPhase {
    Incomplete,
    Reachable,
    Stale,
    Failed,
    Expired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkNeighborState {
    pub interface: u16,
    pub family: AddressFamily,
    pub address: [u8; 16],
    pub link_address: [u8; 8],
    pub link_address_bytes: u8,
    pub generation: u32,
    pub phase: NeighborPhase,
    pub expires_at_tick: Option<u64>,
}

impl NetworkNeighborState {
    pub fn validate(self) -> Result<(), NetworkReason> {
        if self.interface == 0
            || self.generation == 0
            || self.link_address_bytes == 0
            || usize::from(self.link_address_bytes) > self.link_address.len()
            || self.link_address[usize::from(self.link_address_bytes)..]
                .iter()
                .any(|byte| *byte != 0)
            || !address_is_canonical(self.family, &self.address)
        {
            return Err(NetworkReason::Bounds);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutePolicy {
    Local,
    Forward,
    Reject,
    Blackhole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkRouteState {
    pub generation: u32,
    pub family: AddressFamily,
    pub prefix: [u8; 16],
    pub prefix_length: u8,
    pub next_hop: Option<[u8; 16]>,
    pub interface: u16,
    pub mtu: u16,
    pub policy: RoutePolicy,
}

impl NetworkRouteState {
    pub fn validate(self) -> Result<(), NetworkReason> {
        if self.generation == 0
            || self.interface == 0
            || self.mtu == 0
            || !prefix_is_canonical(self.family, &self.prefix, self.prefix_length)
            || self
                .next_hop
                .is_some_and(|address| !address_is_canonical(self.family, &address))
        {
            return Err(NetworkReason::RouteInvalid);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkServiceRegistration {
    pub name: [u8; crate::MAXIMUM_NAME_BYTES],
    pub name_bytes: u8,
    pub family: AddressFamily,
    pub address: [u8; 16],
    pub port: u16,
    pub protocol: crate::TransportProtocol,
    pub generation: u32,
    pub expires_at_tick: u64,
}

impl NetworkServiceRegistration {
    pub fn validate(self) -> Result<(), NetworkReason> {
        if self.name_bytes == 0
            || usize::from(self.name_bytes) > self.name.len()
            || !self.name[..usize::from(self.name_bytes)].is_ascii()
            || self.name[..usize::from(self.name_bytes)].contains(&0)
            || self.name[usize::from(self.name_bytes)..]
                .iter()
                .any(|byte| *byte != 0)
            || self.port == 0
            || self.generation == 0
            || self.expires_at_tick == 0
            || !address_is_canonical(self.family, &self.address)
        {
            return Err(NetworkReason::Bounds);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReachabilityScope {
    LinkLocal,
    LocalNetwork,
    Routed,
    Internet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReachabilityOutcome {
    Reachable,
    Unreachable,
    TimedOut,
    RateLimited,
    Unsupported,
    ProviderLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkReachabilityObservation {
    pub family: AddressFamily,
    pub target: [u8; 16],
    pub scope: ReachabilityScope,
    pub outcome: ReachabilityOutcome,
    pub latency_ticks: Option<u64>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

impl NetworkReachabilityObservation {
    pub fn validate(self, now_tick: u64) -> Result<(), NetworkReason> {
        if !address_is_canonical(self.family, &self.target)
            || self.observed_at_tick > now_tick
            || self.valid_until_tick <= now_tick
        {
            return Err(NetworkReason::ObservationStale);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketDisposition {
    Pending,
    Forwarded,
    LocalDelivery,
    Dropped,
    Rejected,
    NoRoute,
    HopExhausted,
    MtuExceeded,
    PolicyDenied,
    ProviderLost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkPacket {
    pub sequence: u64,
    pub family: AddressFamily,
    pub source: [u8; 16],
    pub destination: [u8; 16],
    pub hop_limit: u8,
    pub fragmented: bool,
    pub egress_interface: Option<u16>,
    pub disposition: PacketDisposition,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatagramDelivery {
    Pending,
    Delivered,
    Lost,
    Duplicated,
    Reordered,
    Rejected,
    Cancelled,
    ProviderLost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkDatagram {
    pub session: Option<u64>,
    pub family: AddressFamily,
    pub source: [u8; 16],
    pub source_port: u16,
    pub destination: [u8; 16],
    pub destination_port: u16,
    pub sequence: u64,
    pub delivery: DatagramDelivery,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamPressure {
    Ready,
    Backpressured,
    Draining,
}

impl NetworkDatagram {
    pub fn validate(&self) -> Result<(), NetworkReason> {
        if self.source_port == 0
            || self.destination_port == 0
            || self.session == Some(0)
            || self.sequence == 0
            || self.payload.is_empty()
            || self.payload.len() > MAXIMUM_DATAGRAM_BYTES
            || !address_is_canonical(self.family, &self.source)
            || !address_is_canonical(self.family, &self.destination)
        {
            return Err(NetworkReason::Bounds);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteStreamChunk {
    pub session: u64,
    pub offset: u64,
    pub eof: bool,
    pub read_half_closed: bool,
    pub write_half_closed: bool,
    pub pressure: StreamPressure,
    pub bytes: Vec<u8>,
}

impl ByteStreamChunk {
    pub fn validate(&self) -> Result<(), NetworkReason> {
        if self.session == 0
            || self.bytes.len() > MAXIMUM_STREAM_CHUNK_BYTES
            || (self.bytes.is_empty()
                && !self.eof
                && !self.read_half_closed
                && !self.write_half_closed)
            || self.offset.checked_add(self.bytes.len() as u64).is_none()
        {
            return Err(NetworkReason::Bounds);
        }
        Ok(())
    }
}

impl NetworkPacket {
    pub fn ipv4(
        sequence: u64,
        source: Ipv4Address,
        destination: Ipv4Address,
        hop_limit: u8,
        payload: Vec<u8>,
    ) -> Result<Self, NetworkReason> {
        let mut source_bytes = [0; 16];
        source_bytes[..4].copy_from_slice(&source.0);
        let mut destination_bytes = [0; 16];
        destination_bytes[..4].copy_from_slice(&destination.0);
        let packet = Self {
            sequence,
            family: AddressFamily::Ipv4,
            source: source_bytes,
            destination: destination_bytes,
            hop_limit,
            fragmented: false,
            egress_interface: None,
            disposition: PacketDisposition::Pending,
            payload,
        };
        packet.validate()?;
        Ok(packet)
    }

    pub fn validate(&self) -> Result<(), NetworkReason> {
        if self.sequence == 0
            || self.egress_interface == Some(0)
            || self.payload.is_empty()
            || self.payload.len() > MAXIMUM_PACKET_BYTES
            || !address_is_canonical(self.family, &self.source)
            || !address_is_canonical(self.family, &self.destination)
        {
            return Err(NetworkReason::Bounds);
        }
        Ok(())
    }
}

fn address_is_canonical(family: AddressFamily, address: &[u8; 16]) -> bool {
    match family {
        AddressFamily::Ipv4 => address[4..].iter().all(|byte| *byte == 0),
        AddressFamily::Ipv6 => true,
    }
}

fn prefix_is_canonical(family: AddressFamily, prefix: &[u8; 16], prefix_length: u8) -> bool {
    let maximum_prefix = match family {
        AddressFamily::Ipv4 => 32,
        AddressFamily::Ipv6 => 128,
    };
    if prefix_length > maximum_prefix || !address_is_canonical(family, prefix) {
        return false;
    }
    let full_bytes = usize::from(prefix_length / 8);
    let remaining = prefix_length % 8;
    if remaining != 0 {
        let host_mask = u8::MAX >> remaining;
        if prefix[full_bytes] & host_mask != 0 {
            return false;
        }
    }
    let first_host_byte = full_bytes + usize::from(remaining != 0);
    let address_bytes = usize::from(maximum_prefix / 8);
    prefix[first_host_byte..address_bytes]
        .iter()
        .all(|byte| *byte == 0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteEntry {
    pub family: AddressFamily,
    pub prefix: [u8; 16],
    pub prefix_length: u8,
    pub egress_interface: u16,
    pub mtu: u16,
    pub forwarding_admitted: bool,
}

impl RouteEntry {
    pub const fn ipv4(
        prefix: Ipv4Address,
        prefix_length: u8,
        egress_interface: u16,
        mtu: u16,
        forwarding_admitted: bool,
    ) -> Self {
        let [a, b, c, d] = prefix.0;
        Self {
            family: AddressFamily::Ipv4,
            prefix: [a, b, c, d, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            prefix_length,
            egress_interface,
            mtu,
            forwarding_admitted,
        }
    }

    pub fn validate(self) -> Result<(), NetworkReason> {
        let maximum_prefix = match self.family {
            AddressFamily::Ipv4 => 32,
            AddressFamily::Ipv6 => 128,
        };
        if self.prefix_length > maximum_prefix
            || !prefix_is_canonical(self.family, &self.prefix, self.prefix_length)
            || self.egress_interface == 0
            || self.mtu == 0
        {
            return Err(NetworkReason::RouteInvalid);
        }
        Ok(())
    }

    fn matches(self, address: &[u8; 16]) -> bool {
        let bits = usize::from(self.prefix_length);
        let full_bytes = bits / 8;
        let remaining = bits % 8;
        if self.prefix[..full_bytes] != address[..full_bytes] {
            return false;
        }
        if remaining == 0 {
            return true;
        }
        let mask = u8::MAX << (8 - remaining);
        self.prefix[full_bytes] & mask == address[full_bytes] & mask
    }
}

pub struct RouteTable {
    routes: [Option<RouteEntry>; MAXIMUM_ROUTES],
    generation: u32,
}

impl Default for RouteTable {
    fn default() -> Self {
        Self::new()
    }
}

impl RouteTable {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            routes: [None; MAXIMUM_ROUTES],
            generation: 1,
        }
    }

    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.routes.iter().flatten().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn install(&mut self, route: RouteEntry) -> Result<u32, NetworkReason> {
        route.validate()?;
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or(NetworkReason::Bounds)?;
        if let Some(existing) = self.routes.iter_mut().flatten().find(|existing| {
            existing.family == route.family
                && existing.prefix == route.prefix
                && existing.prefix_length == route.prefix_length
        }) {
            *existing = route;
        } else {
            let slot = self
                .routes
                .iter_mut()
                .find(|slot| slot.is_none())
                .ok_or(NetworkReason::RouteTableFull)?;
            *slot = Some(route);
        }
        self.generation = next_generation;
        Ok(self.generation)
    }

    pub fn remove(
        &mut self,
        family: AddressFamily,
        prefix: [u8; 16],
        prefix_length: u8,
    ) -> Result<u32, NetworkReason> {
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or(NetworkReason::Bounds)?;
        let route = self
            .routes
            .iter_mut()
            .find(|slot| {
                slot.is_some_and(|route| {
                    route.family == family
                        && route.prefix == prefix
                        && route.prefix_length == prefix_length
                })
            })
            .ok_or(NetworkReason::NoRoute)?;
        *route = None;
        self.generation = next_generation;
        Ok(self.generation)
    }

    #[must_use]
    pub fn select(&self, packet: &NetworkPacket) -> Option<RouteEntry> {
        self.routes
            .iter()
            .flatten()
            .filter(|route| route.family == packet.family && route.matches(&packet.destination))
            .max_by_key(|route| route.prefix_length)
            .copied()
    }

    pub fn forward(&self, mut packet: NetworkPacket) -> NetworkPacket {
        if packet.hop_limit <= 1 {
            packet.disposition = PacketDisposition::HopExhausted;
            return packet;
        }
        let Some(route) = self.select(&packet) else {
            packet.disposition = PacketDisposition::NoRoute;
            return packet;
        };
        if !route.forwarding_admitted {
            packet.disposition = PacketDisposition::PolicyDenied;
            return packet;
        }
        if packet.payload.len() > usize::from(route.mtu) {
            packet.disposition = PacketDisposition::MtuExceeded;
            return packet;
        }
        packet.hop_limit -= 1;
        packet.egress_interface = Some(route.egress_interface);
        packet.disposition = PacketDisposition::Forwarded;
        packet
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionLifecycle {
    Accepted,
    Connected,
    Authenticated,
    HalfClosed,
    Draining,
    Closed,
    TimedOut,
    Reset,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkSession {
    pub identity: u64,
    pub generation: u32,
    pub family: AddressFamily,
    pub protocol: crate::TransportProtocol,
    pub local: [u8; 16],
    pub local_port: u16,
    pub peer: [u8; 16],
    pub peer_port: u16,
    pub lifecycle: SessionLifecycle,
    pub authenticated: bool,
    pub expires_at_tick: u64,
}

pub struct SessionTable {
    sessions: [Option<NetworkSession>; MAXIMUM_SESSIONS],
    next_identity: u64,
    next_generation: u32,
    table_generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionAdmission {
    pub protocol: crate::TransportProtocol,
    pub local: [u8; 16],
    pub local_port: u16,
    pub peer: [u8; 16],
    pub peer_port: u16,
    pub now_tick: u64,
    pub timeout_ticks: u64,
}

impl Default for SessionTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionTable {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sessions: [None; MAXIMUM_SESSIONS],
            next_identity: 1,
            next_generation: 1,
            table_generation: 1,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.iter().flatten().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.table_generation
    }

    #[must_use]
    pub fn next_expiry(&self) -> Option<u64> {
        self.sessions
            .iter()
            .flatten()
            .map(|session| session.expires_at_tick)
            .min()
    }

    pub fn accept(&mut self, admission: SessionAdmission) -> Result<NetworkSession, NetworkReason> {
        if admission.local_port == 0
            || admission.peer_port == 0
            || admission.timeout_ticks == 0
            || !address_is_canonical(AddressFamily::Ipv4, &admission.local)
            || !address_is_canonical(AddressFamily::Ipv4, &admission.peer)
        {
            return Err(NetworkReason::Bounds);
        }
        let next_identity = self
            .next_identity
            .checked_add(1)
            .ok_or(NetworkReason::Bounds)?;
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(NetworkReason::Bounds)?;
        let next_table_generation = self
            .table_generation
            .checked_add(1)
            .ok_or(NetworkReason::Bounds)?;
        let expires_at_tick = admission
            .now_tick
            .checked_add(admission.timeout_ticks)
            .ok_or(NetworkReason::Bounds)?;
        let slot = self
            .sessions
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(NetworkReason::SessionTableFull)?;
        let session = NetworkSession {
            identity: self.next_identity.max(1),
            generation: self.next_generation.max(1),
            family: AddressFamily::Ipv4,
            protocol: admission.protocol,
            local: admission.local,
            local_port: admission.local_port,
            peer: admission.peer,
            peer_port: admission.peer_port,
            lifecycle: SessionLifecycle::Accepted,
            authenticated: false,
            expires_at_tick,
        };
        self.next_identity = next_identity;
        self.next_generation = next_generation;
        self.table_generation = next_table_generation;
        *slot = Some(session);
        Ok(session)
    }

    pub fn transition(
        &mut self,
        identity: u64,
        generation: u32,
        lifecycle: SessionLifecycle,
    ) -> Result<NetworkSession, NetworkReason> {
        let index = self
            .sessions
            .iter()
            .position(|slot| slot.is_some_and(|session| session.identity == identity))
            .ok_or(NetworkReason::SessionMissing)?;
        let mut session = self.sessions[index].expect("located session is present");
        if session.generation != generation {
            return Err(NetworkReason::StaleGeneration);
        }
        session.lifecycle = lifecycle;
        if matches!(
            lifecycle,
            SessionLifecycle::Closed
                | SessionLifecycle::TimedOut
                | SessionLifecycle::Reset
                | SessionLifecycle::Cancelled
                | SessionLifecycle::Failed
        ) {
            let next_table_generation = self
                .table_generation
                .checked_add(1)
                .ok_or(NetworkReason::Bounds)?;
            self.sessions[index] = None;
            self.table_generation = next_table_generation;
            return Ok(session);
        }
        self.sessions[index] = Some(session);
        Ok(session)
    }

    pub fn expire_one(&mut self, now_tick: u64) -> Result<Option<NetworkSession>, NetworkReason> {
        let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.is_some_and(|session| now_tick >= session.expires_at_tick))
        else {
            return Ok(None);
        };
        let next_table_generation = self
            .table_generation
            .checked_add(1)
            .ok_or(NetworkReason::Bounds)?;
        let mut expired = self.sessions[index]
            .take()
            .expect("expired slot was present");
        expired.lifecycle = SessionLifecycle::TimedOut;
        self.table_generation = next_table_generation;
        Ok(Some(expired))
    }

    pub fn expire(&mut self, now_tick: u64) -> Result<usize, NetworkReason> {
        let mut expired = 0;
        while self.expire_one(now_tick)?.is_some() {
            expired += 1;
        }
        Ok(expired)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkControlKind {
    Link,
    Lease,
    Neighbor,
    Route,
    Session,
    Timeout,
    Loss,
    Reset,
    Policy,
    Provider,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkControlOutcome {
    Observed,
    Admitted,
    Applied,
    Rejected,
    Expired,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkControlEvent {
    pub identity: u64,
    pub generation: u32,
    pub kind: NetworkControlKind,
    pub outcome: NetworkControlOutcome,
    pub tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedStatePolicy {
    ReplaceLatest,
    Expiring,
    GenerationFenced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetainedNetworkState {
    pub table: u8,
    pub generation: u32,
    pub items: u16,
    pub bytes: u32,
    pub observed_at_tick: u64,
    pub expires_at_tick: Option<u64>,
    pub policy: RetainedStatePolicy,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn packet(destination: [u8; 4], hop_limit: u8, bytes: usize) -> NetworkPacket {
        NetworkPacket::ipv4(
            1,
            Ipv4Address([10, 0, 0, 2]),
            Ipv4Address(destination),
            hop_limit,
            vec![0x55; bytes],
        )
        .unwrap()
    }

    #[test]
    fn published_network_value_types_are_all_distinct() {
        for (index, left) in NETWORK_VALUE_TYPES.iter().enumerate() {
            for right in &NETWORK_VALUE_TYPES[index + 1..] {
                assert_ne!(left, right);
                assert_ne!(left.contract_id, right.contract_id);
            }
        }
    }

    #[test]
    fn every_published_type_identity_hashes_its_exact_current_descriptor() {
        for (value_type, descriptor) in [
            (LINK_OBSERVATION_TYPE, LINK_OBSERVATION_DESCRIPTOR),
            (FRAME_TYPE, FRAME_DESCRIPTOR),
            (PACKET_TYPE, PACKET_DESCRIPTOR),
            (DATAGRAM_TYPE, DATAGRAM_DESCRIPTOR),
            (BYTE_STREAM_TYPE, BYTE_STREAM_DESCRIPTOR),
            (SESSION_TYPE, SESSION_DESCRIPTOR),
            (CONTROL_EVENT_TYPE, CONTROL_EVENT_DESCRIPTOR),
            (
                RETAINED_NETWORK_STATE_TYPE,
                RETAINED_NETWORK_STATE_DESCRIPTOR,
            ),
            (ADDRESS_STATE_TYPE, ADDRESS_STATE_DESCRIPTOR),
            (DHCP_LEASE_TYPE, DHCP_LEASE_DESCRIPTOR),
            (NEIGHBOR_STATE_TYPE, NEIGHBOR_STATE_DESCRIPTOR),
            (ROUTE_STATE_TYPE, ROUTE_STATE_DESCRIPTOR),
            (SERVICE_REGISTRATION_TYPE, SERVICE_REGISTRATION_DESCRIPTOR),
            (
                REACHABILITY_OBSERVATION_TYPE,
                REACHABILITY_OBSERVATION_DESCRIPTOR,
            ),
        ] {
            assert_eq!(
                value_type.semantic_hash,
                SemanticHash::from_bytes(Sha256::digest(descriptor.as_bytes()).into()),
                "{}",
                value_type.contract_id
            );
        }
    }

    #[test]
    fn frame_datagram_and_stream_bounds_do_not_invent_each_others_semantics() {
        let frame = NetworkFrame {
            interface: 1,
            direction: NetworkDirection::Ingress,
            protocol: Some(0x0800),
            observed_at_tick: 3,
            payload: vec![0; MAXIMUM_FRAME_BYTES],
        };
        assert!(frame.validate().is_ok());
        assert_eq!(
            NetworkFrame {
                payload: vec![0; MAXIMUM_FRAME_BYTES + 1],
                ..frame
            }
            .validate(),
            Err(NetworkReason::Bounds)
        );
        assert_eq!(
            NetworkFrame {
                protocol: Some(0),
                ..frame
            }
            .validate(),
            Err(NetworkReason::Bounds)
        );

        let datagram = NetworkDatagram {
            session: None,
            family: AddressFamily::Ipv4,
            source: [0; 16],
            source_port: 10_000,
            destination: [0; 16],
            destination_port: 20_000,
            sequence: 1,
            delivery: DatagramDelivery::Lost,
            payload: vec![1; 64],
        };
        assert!(datagram.validate().is_ok());
        assert_eq!(datagram.delivery, DatagramDelivery::Lost);
        assert_eq!(
            NetworkDatagram {
                session: Some(0),
                ..datagram.clone()
            }
            .validate(),
            Err(NetworkReason::Bounds)
        );

        let half_close = ByteStreamChunk {
            session: 7,
            offset: 64,
            eof: false,
            read_half_closed: true,
            write_half_closed: false,
            pressure: StreamPressure::Draining,
            bytes: Vec::new(),
        };
        assert!(half_close.validate().is_ok());
        assert_eq!(
            ByteStreamChunk {
                read_half_closed: false,
                ..half_close
            }
            .validate(),
            Err(NetworkReason::Bounds)
        );
    }

    #[test]
    fn canonical_state_values_reject_ambiguous_unused_bytes_and_zero_sentinels() {
        let mut name = [0; crate::MAXIMUM_NAME_BYTES];
        name[..10].copy_from_slice(b"pete.local");
        let registration = NetworkServiceRegistration {
            name,
            name_bytes: 10,
            family: AddressFamily::Ipv4,
            address: [0; 16],
            port: 8080,
            protocol: crate::TransportProtocol::Tcp,
            generation: 1,
            expires_at_tick: 10,
        };
        assert!(registration.validate().is_ok());
        let mut noncanonical_name = registration;
        noncanonical_name.name[10] = b'x';
        assert_eq!(noncanonical_name.validate(), Err(NetworkReason::Bounds));

        let neighbor = NetworkNeighborState {
            interface: 1,
            family: AddressFamily::Ipv4,
            address: [0; 16],
            link_address: [1, 2, 3, 4, 5, 6, 0, 0],
            link_address_bytes: 6,
            generation: 1,
            phase: NeighborPhase::Reachable,
            expires_at_tick: Some(10),
        };
        assert!(neighbor.validate().is_ok());
        let mut noncanonical_link = neighbor;
        noncanonical_link.link_address[7] = 1;
        assert_eq!(noncanonical_link.validate(), Err(NetworkReason::Bounds));

        let mut packet = NetworkPacket::ipv4(
            1,
            Ipv4Address([10, 0, 0, 1]),
            Ipv4Address([10, 0, 0, 2]),
            4,
            vec![1],
        )
        .unwrap();
        packet.egress_interface = Some(0);
        assert_eq!(packet.validate(), Err(NetworkReason::Bounds));
    }

    #[test]
    fn route_selection_and_every_disposition_are_deterministic() {
        let mut routes = RouteTable::new();
        routes
            .install(RouteEntry::ipv4(
                Ipv4Address([10, 0, 0, 0]),
                8,
                1,
                1_500,
                true,
            ))
            .unwrap();
        routes
            .install(RouteEntry::ipv4(
                Ipv4Address([10, 1, 0, 0]),
                16,
                2,
                512,
                true,
            ))
            .unwrap();

        let forwarded = routes.forward(packet([10, 1, 2, 3], 4, 64));
        assert_eq!(forwarded.disposition, PacketDisposition::Forwarded);
        assert_eq!(forwarded.egress_interface, Some(2));
        assert_eq!(forwarded.hop_limit, 3);
        assert_eq!(
            routes.forward(packet([10, 1, 2, 3], 1, 64)).disposition,
            PacketDisposition::HopExhausted
        );
        assert_eq!(
            routes.forward(packet([192, 0, 2, 1], 4, 64)).disposition,
            PacketDisposition::NoRoute
        );
        assert_eq!(
            routes.forward(packet([10, 1, 2, 3], 4, 513)).disposition,
            PacketDisposition::MtuExceeded
        );

        routes
            .install(RouteEntry::ipv4(
                Ipv4Address([203, 0, 113, 0]),
                24,
                3,
                1_500,
                false,
            ))
            .unwrap();
        assert_eq!(
            routes.forward(packet([203, 0, 113, 7], 4, 64)).disposition,
            PacketDisposition::PolicyDenied
        );
    }

    #[test]
    fn route_table_replacement_and_removal_fence_generations() {
        let mut routes = RouteTable::new();
        let first = RouteEntry::ipv4(Ipv4Address([10, 0, 0, 0]), 8, 1, 1_500, true);
        let generation = routes.install(first).unwrap();
        assert_eq!(routes.len(), 1);
        let replacement = RouteEntry {
            mtu: 1_200,
            ..first
        };
        assert!(routes.install(replacement).unwrap() > generation);
        assert_eq!(routes.len(), 1);
        assert_eq!(
            routes.select(&packet([10, 1, 2, 3], 4, 64)),
            Some(replacement)
        );
        assert!(
            routes
                .remove(AddressFamily::Ipv4, first.prefix, first.prefix_length)
                .unwrap()
                > generation
        );
        assert!(routes.is_empty());
    }

    #[test]
    fn sessions_are_finite_expire_and_reject_stale_events() {
        let mut sessions = SessionTable::new();
        let mut peer_address = [0; 16];
        peer_address[..4].copy_from_slice(&[10, 0, 0, 2]);
        let first = sessions
            .accept(SessionAdmission {
                protocol: crate::TransportProtocol::Tcp,
                local: [0; 16],
                local_port: 8080,
                peer: peer_address,
                peer_port: 40_000,
                now_tick: 10,
                timeout_ticks: 5,
            })
            .unwrap();
        assert_eq!(
            sessions.transition(
                first.identity,
                first.generation + 1,
                SessionLifecycle::Connected
            ),
            Err(NetworkReason::StaleGeneration)
        );
        assert_eq!(
            sessions
                .transition(
                    first.identity,
                    first.generation,
                    SessionLifecycle::Connected
                )
                .unwrap()
                .lifecycle,
            SessionLifecycle::Connected
        );
        assert_eq!(sessions.expire(15).unwrap(), 1);
        assert!(sessions.is_empty());

        for peer_index in 0..MAXIMUM_SESSIONS {
            sessions
                .accept(SessionAdmission {
                    protocol: crate::TransportProtocol::Tcp,
                    local: [0; 16],
                    local_port: 8080,
                    peer: peer_address,
                    peer_port: 40_000 + peer_index as u16,
                    now_tick: 20,
                    timeout_ticks: 5,
                })
                .unwrap();
        }
        assert_eq!(
            sessions.accept(SessionAdmission {
                protocol: crate::TransportProtocol::Tcp,
                local: [0; 16],
                local_port: 8080,
                peer: peer_address,
                peer_port: 50_000,
                now_tick: 20,
                timeout_ticks: 5,
            }),
            Err(NetworkReason::SessionTableFull)
        );
    }
}
