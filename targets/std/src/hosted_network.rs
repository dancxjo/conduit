//! Std resolver and socket realization for portable application-network Info.

use conduit_form::rust_binding::BoundedSequence;
use conduit_net::{
    DnsQuery, DnsRecordKind, DnsResult, DnsTtl, NetworkAddress, NetworkConnectionState,
    NetworkEndpoint, NetworkReason, NetworkTransport, ResolvedNetworkAddress,
    ResolvedNetworkEndpoint, NETWORK_MAXIMUM_CANDIDATES,
};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointFreshness {
    Current,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkProviderAvailability {
    Available,
    Lost,
}

pub fn resolve_dns(query: &DnsQuery) -> DnsResult {
    let resolved = match (query.name().as_str(), *query.port()).to_socket_addrs() {
        Ok(resolved) => resolved,
        Err(error) => return dns_refused(format!("resolver refused query: {error}")),
    };
    let mut candidates = Vec::with_capacity(NETWORK_MAXIMUM_CANDIDATES);
    for address in resolved {
        if !record_matches(*query.record_kind(), address.ip()) {
            continue;
        }
        let endpoint = resolved_socket_endpoint(address, *query.transport());
        if !candidates.contains(&endpoint) {
            candidates.push(endpoint);
        }
        if candidates.len() == NETWORK_MAXIMUM_CANDIDATES {
            break;
        }
    }
    if candidates.is_empty() {
        return dns_refused("resolver returned no matching address records".to_string());
    }
    DnsResult::current(
        BoundedSequence::try_from_iter(candidates).expect("resolver candidate bound enforced"),
        query.name().clone(),
        // `ToSocketAddrs` does not expose authoritative TTL. Do not invent it.
        DnsTtl::Unavailable,
    )
    .expect("validated DNS query and bounded candidates form a DNS result")
}

pub fn resolve_dns_with_provider(
    query: &DnsQuery,
    provider: NetworkProviderAvailability,
) -> DnsResult {
    match provider {
        NetworkProviderAvailability::Available => resolve_dns(query),
        NetworkProviderAvailability::Lost => {
            DnsResult::provider_lost(network_reason("resolver provider unavailable".to_string()))
                .expect("static provider-loss reason is bounded")
        }
    }
}

pub fn connect_tcp(
    endpoint: &NetworkEndpoint,
    freshness: EndpointFreshness,
    provider: NetworkProviderAvailability,
    timeout: Duration,
) -> Vec<NetworkConnectionState> {
    if *endpoint.transport() != NetworkTransport::Tcp {
        return vec![connection_refused(
            "endpoint is not an admitted TCP endpoint".to_string(),
        )];
    }
    if provider == NetworkProviderAvailability::Lost {
        return vec![connection_lost(
            "connection provider unavailable".to_string(),
        )];
    }
    if freshness == EndpointFreshness::Stale {
        return vec![NetworkConnectionState::stale_endpoint(
            endpoint.address().clone(),
            *endpoint.port(),
            *endpoint.transport(),
        )
        .expect("admitted endpoint remains valid")];
    }

    let mut lifecycle = vec![NetworkConnectionState::requested(
        endpoint.address().clone(),
        *endpoint.port(),
        *endpoint.transport(),
    )
    .expect("admitted endpoint remains valid")];
    let address = match socket_address(endpoint) {
        Some(address) => address,
        None => {
            lifecycle.push(connection_refused(
                "DNS names must be resolved before connecting".to_string(),
            ));
            return lifecycle;
        }
    };
    lifecycle.push(
        NetworkConnectionState::connecting(
            endpoint.address().clone(),
            *endpoint.port(),
            *endpoint.transport(),
        )
        .expect("admitted endpoint remains valid"),
    );
    match TcpStream::connect_timeout(&address, timeout) {
        Ok(stream) => {
            let local = stream.local_addr().ok();
            let peer = stream.peer_addr().ok();
            match (local, peer) {
                (Some(local), Some(peer)) => {
                    lifecycle.push(
                        NetworkConnectionState::connected(
                            socket_endpoint(local, NetworkTransport::Tcp),
                            socket_endpoint(peer, NetworkTransport::Tcp),
                        )
                        .expect("observed socket endpoints are valid"),
                    );
                    drop(stream);
                    lifecycle.push(NetworkConnectionState::Closed);
                }
                _ => lifecycle.push(connection_lost(
                    "socket endpoint observation was lost".to_string(),
                )),
            }
        }
        Err(error) => lifecycle.push(connection_refused(format!("connection refused: {error}"))),
    }
    lifecycle
}

fn record_matches(kind: DnsRecordKind, address: IpAddr) -> bool {
    matches!(
        (kind, address),
        (DnsRecordKind::A, IpAddr::V4(_))
            | (DnsRecordKind::Aaaa, IpAddr::V6(_))
            | (DnsRecordKind::Address, _)
    )
}

fn socket_address(endpoint: &NetworkEndpoint) -> Option<SocketAddr> {
    let address = match endpoint.address() {
        NetworkAddress::Ipv4(octets) => IpAddr::V4((*octets).into()),
        NetworkAddress::Ipv6(octets) => IpAddr::V6((*octets).into()),
        NetworkAddress::DnsName(_) => return None,
    };
    Some(SocketAddr::new(address, *endpoint.port()))
}

fn socket_endpoint(address: SocketAddr, transport: NetworkTransport) -> NetworkEndpoint {
    let port = address.port();
    let address = match address.ip() {
        IpAddr::V4(value) => NetworkAddress::Ipv4(value.octets()),
        IpAddr::V6(value) => NetworkAddress::Ipv6(value.octets()),
    };
    NetworkEndpoint::new(address, port, transport).expect("socket endpoint has a nonzero port")
}

fn resolved_socket_endpoint(
    address: SocketAddr,
    transport: NetworkTransport,
) -> ResolvedNetworkEndpoint {
    let port = address.port();
    let address = match address.ip() {
        IpAddr::V4(value) => ResolvedNetworkAddress::Ipv4(value.octets()),
        IpAddr::V6(value) => ResolvedNetworkAddress::Ipv6(value.octets()),
    };
    ResolvedNetworkEndpoint::new(address, port, transport)
        .expect("resolved socket endpoint has a nonzero port")
}

fn network_reason(reason: String) -> NetworkReason {
    NetworkReason::new(reason).expect("host network reasons fit the portable bound")
}

fn dns_refused(reason: String) -> DnsResult {
    DnsResult::refused(network_reason(reason)).expect("bounded refusal forms a DNS result")
}

fn connection_refused(reason: String) -> NetworkConnectionState {
    NetworkConnectionState::refused(network_reason(reason))
        .expect("bounded refusal forms a connection observation")
}

fn connection_lost(reason: String) -> NetworkConnectionState {
    NetworkConnectionState::lost(network_reason(reason))
        .expect("bounded loss forms a connection observation")
}
