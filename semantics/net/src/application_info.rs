//! Portable application-network Info below HTTP and separate from Conduit Lines.

use crate::{ApplicationNetworkRefusal, DnsQuery, DnsRecordKind, DnsTtl, NetworkTransport};
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{kind_id, StructuredFieldType, StructuredInfoType, StructuredVariantCase};

pub const NETWORK_ENDPOINT_TYPE: &str = "NetworkEndpoint";
pub const DNS_QUERY_TYPE: &str = "DnsQuery";
pub const DNS_RESULT_TYPE: &str = "DnsResult";
pub const NETWORK_CONNECTION_STATE_TYPE: &str = "NetworkConnectionState";
pub const NETWORK_MAXIMUM_CANDIDATES: usize = 4;
pub const NETWORK_MAXIMUM_NAME_BYTES: usize = 253;

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
    Refused {
        reason: String,
    },
    ProviderLost {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkConnectionState {
    Requested {
        endpoint: NetworkEndpoint,
    },
    Resolving {
        name: String,
    },
    Connecting {
        endpoint: NetworkEndpoint,
    },
    Connected {
        local: NetworkEndpoint,
        peer: NetworkEndpoint,
    },
    StaleEndpoint {
        endpoint: NetworkEndpoint,
    },
    Refused {
        reason: String,
    },
    Lost {
        reason: String,
    },
    Closed,
}

impl DnsQuery {
    pub fn validate(&self) -> Result<(), ApplicationNetworkRefusal> {
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
            if !matches!(
                &candidate.address,
                NetworkAddress::Ipv4(_) | NetworkAddress::Ipv6(_)
            ) {
                return Err(ApplicationNetworkRefusal::CandidateTransportMismatch);
            }
        }
        Ok(())
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
    leaf("value/unit")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text")
}

fn count_type() -> StructuredInfoType {
    leaf("value/count")
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

pub fn dns_query_type() -> StructuredInfoType {
    DnsQuery::semantic_type().expect("checked native DNS query Type")
}

fn dns_resolution_type() -> StructuredInfoType {
    let candidate = StructuredInfoType::variant(
        kind_id("net/optional-endpoint@1"),
        vec![
            case("absent", unit_type()),
            case("endpoint", network_endpoint_type()),
        ],
    )
    .expect("reviewed optional endpoint");
    let ttl = StructuredInfoType::variant(
        kind_id("net/dns-ttl@1"),
        vec![
            case("known_seconds", count_type()),
            case("unavailable", unit_type()),
        ],
    )
    .expect("reviewed DNS TTL");
    record(
        "net/dns-resolution@1",
        vec![
            field(
                "candidates",
                StructuredInfoType::collection(candidate, Some(NETWORK_MAXIMUM_CANDIDATES as u16))
                    .expect("bounded DNS candidates"),
            ),
            field("canonical_name", text_type()),
            field("ttl", ttl),
        ],
    )
}

pub fn dns_result_type() -> StructuredInfoType {
    let reason = record("net/network-refusal@1", vec![field("reason", text_type())]);
    let stale = record(
        "net/stale-dns-resolution@1",
        vec![
            field("age_seconds", count_type()),
            field("resolution", dns_resolution_type()),
        ],
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
    let reason = record(
        "net/connection-reason@1",
        vec![field("reason", text_type())],
    );
    let connected = record(
        "net/connected-endpoints@1",
        vec![
            field("local", network_endpoint_type()),
            field("peer", network_endpoint_type()),
        ],
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

pub fn application_network_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (NETWORK_ENDPOINT_TYPE, network_endpoint_type()),
        (DNS_QUERY_TYPE, dns_query_type()),
        (DNS_RESULT_TYPE, dns_result_type()),
        (
            NETWORK_CONNECTION_STATE_TYPE,
            network_connection_state_type(),
        ),
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
        ttl: DnsTtl::known_seconds(30).expect("fixture TTL is valid"),
    };
    (
        DnsQuery::new(
            "fixture.local".into(),
            7,
            DnsRecordKind::Address,
            NetworkTransport::Tcp,
        )
        .expect("bounded fixture DNS query"),
        DnsResult::Stale {
            resolution,
            age_seconds: 31,
        },
        endpoint,
    )
}
