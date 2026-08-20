//! Std resolver and socket realization for portable application-network Info.

use conduit_net::{
    ApplicationNetworkRefusal, DnsQuery, DnsRecordKind, DnsResolution, DnsResult, DnsTtl,
    NetworkAddress, NetworkConnectionState, NetworkEndpoint, NetworkTransport,
    NETWORK_MAXIMUM_CANDIDATES,
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
    if let Err(error) = query.validate() {
        return DnsResult::Refused {
            reason: refusal_message(error),
        };
    }
    let resolved = match (query.name.as_str(), query.port).to_socket_addrs() {
        Ok(resolved) => resolved,
        Err(error) => {
            return DnsResult::Refused {
                reason: format!("resolver refused query: {error}"),
            }
        }
    };
    let mut candidates = Vec::with_capacity(NETWORK_MAXIMUM_CANDIDATES);
    for address in resolved {
        if !record_matches(query.record_kind, address.ip()) {
            continue;
        }
        let endpoint = socket_endpoint(address, NetworkTransport::Tcp);
        if !candidates.contains(&endpoint) {
            candidates.push(endpoint);
        }
        if candidates.len() == NETWORK_MAXIMUM_CANDIDATES {
            break;
        }
    }
    if candidates.is_empty() {
        return DnsResult::Refused {
            reason: "resolver returned no matching address records".to_string(),
        };
    }
    DnsResult::Current(DnsResolution {
        canonical_name: query.name.clone(),
        candidates,
        // `ToSocketAddrs` does not expose authoritative TTL. Do not invent it.
        ttl: DnsTtl::Unavailable,
    })
}

pub fn resolve_dns_with_provider(
    query: &DnsQuery,
    provider: NetworkProviderAvailability,
) -> DnsResult {
    match provider {
        NetworkProviderAvailability::Available => resolve_dns(query),
        NetworkProviderAvailability::Lost => DnsResult::ProviderLost {
            reason: "resolver provider unavailable".to_string(),
        },
    }
}

pub fn connect_tcp(
    endpoint: &NetworkEndpoint,
    freshness: EndpointFreshness,
    provider: NetworkProviderAvailability,
    timeout: Duration,
) -> Vec<NetworkConnectionState> {
    if endpoint.validate().is_err() || endpoint.transport != NetworkTransport::Tcp {
        return vec![NetworkConnectionState::Refused {
            reason: "endpoint is not an admitted TCP endpoint".to_string(),
        }];
    }
    if provider == NetworkProviderAvailability::Lost {
        return vec![NetworkConnectionState::Lost {
            reason: "connection provider unavailable".to_string(),
        }];
    }
    if freshness == EndpointFreshness::Stale {
        return vec![NetworkConnectionState::StaleEndpoint {
            endpoint: endpoint.clone(),
        }];
    }

    let mut lifecycle = vec![NetworkConnectionState::Requested {
        endpoint: endpoint.clone(),
    }];
    let address = match socket_address(endpoint) {
        Some(address) => address,
        None => {
            lifecycle.push(NetworkConnectionState::Refused {
                reason: "DNS names must be resolved before connecting".to_string(),
            });
            return lifecycle;
        }
    };
    lifecycle.push(NetworkConnectionState::Connecting {
        endpoint: endpoint.clone(),
    });
    match TcpStream::connect_timeout(&address, timeout) {
        Ok(stream) => {
            let local = stream.local_addr().ok();
            let peer = stream.peer_addr().ok();
            match (local, peer) {
                (Some(local), Some(peer)) => {
                    lifecycle.push(NetworkConnectionState::Connected {
                        local: socket_endpoint(local, NetworkTransport::Tcp),
                        peer: socket_endpoint(peer, NetworkTransport::Tcp),
                    });
                    drop(stream);
                    lifecycle.push(NetworkConnectionState::Closed);
                }
                _ => lifecycle.push(NetworkConnectionState::Lost {
                    reason: "socket endpoint observation was lost".to_string(),
                }),
            }
        }
        Err(error) => lifecycle.push(NetworkConnectionState::Refused {
            reason: format!("connection refused: {error}"),
        }),
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
    let address = match &endpoint.address {
        NetworkAddress::Ipv4(octets) => IpAddr::V4((*octets).into()),
        NetworkAddress::Ipv6(octets) => IpAddr::V6((*octets).into()),
        NetworkAddress::DnsName(_) => return None,
    };
    Some(SocketAddr::new(address, endpoint.port))
}

fn socket_endpoint(address: SocketAddr, transport: NetworkTransport) -> NetworkEndpoint {
    let port = address.port();
    let address = match address.ip() {
        IpAddr::V4(value) => NetworkAddress::Ipv4(value.octets()),
        IpAddr::V6(value) => NetworkAddress::Ipv6(value.octets()),
    };
    NetworkEndpoint {
        address,
        port,
        transport,
    }
}

fn refusal_message(error: ApplicationNetworkRefusal) -> String {
    format!("invalid DNS query: {error:?}")
}
