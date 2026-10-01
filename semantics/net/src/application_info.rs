//! Portable application-network Info below HTTP and separate from Conduit Lines.

use crate::{
    DnsQuery, DnsRecordKind, DnsResolution, DnsResult, DnsTtl, NetworkAddress,
    NetworkConnectionState, NetworkEndpoint, NetworkTransport, ResolvedNetworkAddress,
    ResolvedNetworkEndpoint,
};
use alloc::{vec, vec::Vec};
use conduit_core::StructuredInfoType;
use conduit_form::rust_binding::BoundedSequence;

pub const NETWORK_ENDPOINT_TYPE: &str = "NetworkEndpoint";
pub const DNS_QUERY_TYPE: &str = "DnsQuery";
pub const DNS_RESULT_TYPE: &str = "DnsResult";
pub const NETWORK_CONNECTION_STATE_TYPE: &str = "NetworkConnectionState";
pub const NETWORK_MAXIMUM_CANDIDATES: usize = 4;
pub const NETWORK_MAXIMUM_NAME_BYTES: usize = 253;

pub fn network_address_type() -> StructuredInfoType {
    NetworkAddress::semantic_type().expect("checked native network address Type")
}

pub fn network_endpoint_type() -> StructuredInfoType {
    NetworkEndpoint::semantic_type().expect("checked native network endpoint Type")
}

pub fn dns_query_type() -> StructuredInfoType {
    DnsQuery::semantic_type().expect("checked native DNS query Type")
}

pub fn dns_result_type() -> StructuredInfoType {
    DnsResult::semantic_type().expect("checked native DNS result Type")
}

pub fn network_connection_state_type() -> StructuredInfoType {
    NetworkConnectionState::semantic_type().expect("checked native connection-state Type")
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
    let endpoint = NetworkEndpoint::new(
        NetworkAddress::Ipv4([127, 0, 0, 1]),
        7,
        NetworkTransport::Tcp,
    )
    .expect("fixture endpoint is valid");
    let resolution = DnsResolution::new(
        BoundedSequence::try_from_iter([ResolvedNetworkEndpoint::new(
            ResolvedNetworkAddress::Ipv4([127, 0, 0, 1]),
            7,
            NetworkTransport::Tcp,
        )
        .expect("fixture resolved endpoint is valid")])
        .expect("one fixture DNS candidate fits"),
        "fixture.local".into(),
        DnsTtl::known_seconds(30).expect("fixture TTL is valid"),
    )
    .expect("bounded fixture DNS resolution");
    (
        DnsQuery::new(
            "fixture.local".into(),
            7,
            DnsRecordKind::Address,
            NetworkTransport::Tcp,
        )
        .expect("bounded fixture DNS query"),
        DnsResult::stale(31, resolution).expect("bounded stale DNS result"),
        endpoint,
    )
}
