//! Portable application-network Info below HTTP and separate from Conduit Lines.

use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, BoundedResourceRef, StructuredFieldType, StructuredInfoType,
    StructuredVariantCase, RESOURCE_REFERENCE_INFO_ID,
};

pub const NETWORK_ENDPOINT_TYPE: &str = "NetworkEndpoint";
pub const DNS_QUERY_TYPE: &str = "DnsQuery";
pub const DNS_RESULT_TYPE: &str = "DnsResult";
pub const NETWORK_CONNECTION_STATE_TYPE: &str = "NetworkConnectionState";
pub const NETWORK_CHUNK_METADATA_TYPE: &str = "NetworkChunkMetadata";
pub const NETWORK_FRAME_TYPE: &str = "NetworkProtocolFrame";
pub const NETWORK_MAXIMUM_CANDIDATES: usize = 4;
pub const NETWORK_MAXIMUM_NAME_BYTES: usize = 253;
pub const NETWORK_MAXIMUM_INLINE_PAYLOAD_BYTES: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkTransport {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum NetworkAddress {
    DnsName(String),
    Ipv4([u8; 4]),
    Ipv6([u8; 16]),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkEndpoint {
    pub address: NetworkAddress,
    pub port: u16,
    pub transport: NetworkTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsRecordKind {
    A,
    Aaaa,
    Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsQuery {
    pub name: String,
    pub port: u16,
    pub record_kind: DnsRecordKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsTtl {
    KnownSeconds(u32),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsResolution {
    pub canonical_name: String,
    pub candidates: Vec<NetworkEndpoint>,
    pub ttl: DnsTtl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsResult {
    Current(DnsResolution),
    Stale {
        resolution: DnsResolution,
        age_seconds: u64,
    },
    Refused { reason: String },
    ProviderLost { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkConnectionState {
    Requested { endpoint: NetworkEndpoint },
    Resolving { name: String },
    Connecting { endpoint: NetworkEndpoint },
    Connected {
        local: NetworkEndpoint,
        peer: NetworkEndpoint,
    },
    StaleEndpoint { endpoint: NetworkEndpoint },
    Refused { reason: String },
    Lost { reason: String },
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkFrameProtocol {
    EchoV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkFrameDirection {
    Received,
    Sent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkFramePayload {
    Inline(Vec<u8>),
    Resource(BoundedResourceRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkChunkShape {
    Datagram,
    StreamChunk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkChunkMetadata {
    pub shape: NetworkChunkShape,
    pub direction: NetworkFrameDirection,
    pub sequence: u64,
    pub payload_bytes: u32,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkProtocolFrame {
    pub protocol: NetworkFrameProtocol,
    pub direction: NetworkFrameDirection,
    pub sequence: u64,
    pub payload: NetworkFramePayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationNetworkRefusal {
    EmptyName,
    NameTooLarge,
    InvalidPort,
    TooManyCandidates,
    CandidateTransportMismatch,
    InlinePayloadTooLarge,
    InvalidResource,
}

impl DnsQuery {
    pub fn validate(&self) -> Result<(), ApplicationNetworkRefusal> {
        validate_name(&self.name)?;
        if self.port == 0 {
            return Err(ApplicationNetworkRefusal::InvalidPort);
        }
        Ok(())
    }
}

impl NetworkEndpoint {
    pub fn validate(&self) -> Result<(), ApplicationNetworkRefusal> {
        if self.port == 0 {
            return Err(ApplicationNetworkRefusal::InvalidPort);
        }
        if let NetworkAddress::DnsName(name) = &self.address {
            validate_name(name)?;
        }
        Ok(())
    }
}

impl DnsResolution {
    pub fn validate(&self) -> Result<(), ApplicationNetworkRefusal> {
        validate_name(&self.canonical_name)?;
        if self.candidates.len() > NETWORK_MAXIMUM_CANDIDATES {
            return Err(ApplicationNetworkRefusal::TooManyCandidates);
        }
        for candidate in &self.candidates {
            candidate.validate()?;
            if !matches!(candidate.address, NetworkAddress::Ipv4(_) | NetworkAddress::Ipv6(_)) {
                return Err(ApplicationNetworkRefusal::CandidateTransportMismatch);
            }
        }
        Ok(())
    }
}

impl NetworkProtocolFrame {
    pub fn validate(&self) -> Result<(), ApplicationNetworkRefusal> {
        match &self.payload {
            NetworkFramePayload::Inline(bytes)
                if bytes.len() > NETWORK_MAXIMUM_INLINE_PAYLOAD_BYTES =>
            {
                Err(ApplicationNetworkRefusal::InlinePayloadTooLarge)
            }
            NetworkFramePayload::Inline(_) => Ok(()),
            NetworkFramePayload::Resource(reference) => reference
                .validate()
                .map_err(|_| ApplicationNetworkRefusal::InvalidResource),
        }
    }
}

impl NetworkChunkMetadata {
    pub fn validate(&self) -> Result<(), ApplicationNetworkRefusal> {
        if self.payload_bytes as usize > NETWORK_MAXIMUM_INLINE_PAYLOAD_BYTES {
            Err(ApplicationNetworkRefusal::InlinePayloadTooLarge)
        } else {
            Ok(())
        }
    }
}

fn validate_name(name: &str) -> Result<(), ApplicationNetworkRefusal> {
    if name.is_empty() {
        Err(ApplicationNetworkRefusal::EmptyName)
    } else if name.len() > NETWORK_MAXIMUM_NAME_BYTES {
        Err(ApplicationNetworkRefusal::NameTooLarge)
    } else {
        Ok(())
    }
}

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed network leaf")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed network field")
}

fn case(name: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload_type).expect("reviewed network case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed network record")
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit@1")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text@1")
}

fn count_type() -> StructuredInfoType {
    leaf("value/count@1")
}

pub fn network_address_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("net/address@1"),
        vec![
            case("dns_name", text_type()),
            case("ipv4", leaf("net/ipv4-octets@1")),
            case("ipv6", leaf("net/ipv6-octets@1")),
        ],
    )
    .expect("reviewed network address")
}

fn network_transport_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("net/transport@1"),
        vec![case("tcp", unit_type()), case("udp", unit_type())],
    )
    .expect("reviewed network transport")
}

pub fn network_endpoint_type() -> StructuredInfoType {
    record(
        "net/endpoint@1",
        vec![
            field("address", network_address_type()),
            field("port", leaf("net/port@1")),
            field("transport", network_transport_type()),
        ],
    )
}

fn dns_record_kind_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("net/dns-record-kind@1"),
        vec![case("a", unit_type()), case("aaaa", unit_type()), case("address", unit_type())],
    )
    .expect("reviewed DNS record kind")
}

pub fn dns_query_type() -> StructuredInfoType {
    record(
        "net/dns-query@1",
        vec![
            field("name", text_type()),
            field("port", leaf("net/port@1")),
            field("record_kind", dns_record_kind_type()),
        ],
    )
}

fn dns_resolution_type() -> StructuredInfoType {
    let candidate = StructuredInfoType::variant(
        kind_id("net/optional-endpoint@1"),
        vec![case("absent", unit_type()), case("endpoint", network_endpoint_type())],
    )
    .expect("reviewed optional endpoint");
    let ttl = StructuredInfoType::variant(
        kind_id("net/dns-ttl@1"),
        vec![case("known_seconds", count_type()), case("unavailable", unit_type())],
    )
    .expect("reviewed DNS TTL") ;
    record(
        "net/dns-resolution@1",
        vec![
            field("candidates", StructuredInfoType::collection(candidate, Some(NETWORK_MAXIMUM_CANDIDATES as u16)).expect("bounded DNS candidates")),
            field("canonical_name", text_type()),
            field("ttl", ttl),
        ],
    )
}

pub fn dns_result_type() -> StructuredInfoType {
    let reason = record("net/network-refusal@1", vec![field("reason", text_type())]);
    let stale = record(
        "net/stale-dns-resolution@1",
        vec![field("age_seconds", count_type()), field("resolution", dns_resolution_type())],
    );
    StructuredInfoType::variant(
        kind_id("net/dns-result@1"),
        vec![
            case("current", dns_resolution_type()),
            case("provider_lost", reason.clone()),
            case("refused", reason),
            case("stale", stale),
        ],
    )
    .expect("reviewed DNS result")
}

pub fn network_connection_state_type() -> StructuredInfoType {
    let reason = record("net/connection-reason@1", vec![field("reason", text_type())]);
    let connected = record(
        "net/connected-endpoints@1",
        vec![field("local", network_endpoint_type()), field("peer", network_endpoint_type())],
    );
    StructuredInfoType::variant(
        kind_id("net/connection-state@1"),
        vec![
            case("closed", unit_type()),
            case("connected", connected),
            case("connecting", network_endpoint_type()),
            case("lost", reason.clone()),
            case("refused", reason),
            case("requested", network_endpoint_type()),
            case("resolving", text_type()),
            case("stale_endpoint", network_endpoint_type()),
        ],
    )
    .expect("reviewed connection state")
}

fn frame_direction_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("net/frame-direction@1"),
        vec![case("received", unit_type()), case("sent", unit_type())],
    )
    .expect("reviewed frame direction")
}

pub fn network_frame_type() -> StructuredInfoType {
    let protocol = StructuredInfoType::variant(
        kind_id("net/frame-protocol@1"),
        vec![case("echo_v1", unit_type())],
    )
    .expect("reviewed frame protocol");
    let payload = StructuredInfoType::variant(
        kind_id("net/frame-payload@1"),
        vec![case("inline", leaf("value/bytes@1")), case("resource", leaf(RESOURCE_REFERENCE_INFO_ID))],
    )
    .expect("reviewed frame payload");
    record(
        "net/protocol-frame@1",
        vec![
            field("direction", frame_direction_type()),
            field("payload", payload),
            field("protocol", protocol),
            field("sequence", count_type()),
        ],
    )
}

pub fn network_chunk_metadata_type() -> StructuredInfoType {
    let shape = StructuredInfoType::variant(
        kind_id("net/chunk-shape@1"),
        vec![case("datagram", unit_type()), case("stream_chunk", unit_type())],
    )
    .expect("reviewed chunk shape");
    record(
        "net/chunk-metadata@1",
        vec![
            field("direction", frame_direction_type()),
            field("payload_bytes", count_type()),
            field("sequence", count_type()),
            field("shape", shape),
            field("truncated", leaf(conduit_core::BOOL_INFO_ID)),
        ],
    )
}

pub fn application_network_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (NETWORK_ENDPOINT_TYPE, network_endpoint_type()),
        (DNS_QUERY_TYPE, dns_query_type()),
        (DNS_RESULT_TYPE, dns_result_type()),
        (NETWORK_CONNECTION_STATE_TYPE, network_connection_state_type()),
        (NETWORK_CHUNK_METADATA_TYPE, network_chunk_metadata_type()),
        (NETWORK_FRAME_TYPE, network_frame_type()),
    ]
}

pub fn deterministic_network_fixture() -> (DnsQuery, DnsResult, NetworkEndpoint) {
    let endpoint = NetworkEndpoint {
        address: NetworkAddress::Ipv4([127, 0, 0, 1]),
        port: 7,
        transport: NetworkTransport::Tcp,
    };
    let resolution = DnsResolution {
        canonical_name: "fixture.local".into(),
        candidates: vec![endpoint.clone()],
        ttl: DnsTtl::KnownSeconds(30),
    };
    (
        DnsQuery { name: "fixture.local".into(), port: 7, record_kind: DnsRecordKind::Address },
        DnsResult::Stale { resolution, age_seconds: 31 },
        endpoint,
    )
}
