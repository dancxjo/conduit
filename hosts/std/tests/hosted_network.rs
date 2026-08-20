use conduit_net::{
    DnsQuery, DnsRecordKind, DnsResult, DnsTtl, NetworkAddress, NetworkConnectionState,
    NetworkEndpoint, NetworkFrameDirection, NetworkFramePayload, NetworkFrameProtocol,
    NetworkProtocolFrame, NetworkTransport, NETWORK_MAXIMUM_INLINE_PAYLOAD_BYTES,
};
use conduit_std_host::hosted_network::{
    connect_tcp, resolve_dns, resolve_dns_with_provider, EndpointFreshness,
    NetworkProviderAvailability,
};
use std::net::{Ipv4Addr, TcpListener};
use std::thread;
use std::time::Duration;

#[test]
fn std_resolver_emits_typed_candidates_without_inventing_ttl() {
    let result = resolve_dns(&DnsQuery {
        name: "localhost".to_string(),
        port: 80,
        record_kind: DnsRecordKind::Address,
    });
    let DnsResult::Current(resolution) = result else {
        panic!("localhost should resolve")
    };
    assert!(!resolution.candidates.is_empty());
    assert_eq!(resolution.ttl, DnsTtl::Unavailable);
    assert!(resolution.candidates.iter().all(|candidate| matches!(
        candidate.address,
        NetworkAddress::Ipv4(_) | NetworkAddress::Ipv6(_)
    )));
}

#[test]
fn std_connection_lifecycle_uses_endpoints_not_socket_integers() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let accept = thread::spawn(move || listener.accept().unwrap());
    let endpoint = NetworkEndpoint {
        address: NetworkAddress::Ipv4(Ipv4Addr::LOCALHOST.octets()),
        port: address.port(),
        transport: NetworkTransport::Tcp,
    };
    let lifecycle = connect_tcp(
        &endpoint,
        EndpointFreshness::Current,
        NetworkProviderAvailability::Available,
        Duration::from_secs(1),
    );
    let _ = accept.join().unwrap();
    assert!(matches!(lifecycle[0], NetworkConnectionState::Requested { .. }));
    assert!(matches!(lifecycle[1], NetworkConnectionState::Connecting { .. }));
    assert!(matches!(lifecycle[2], NetworkConnectionState::Connected { .. }));
    assert_eq!(lifecycle.last(), Some(&NetworkConnectionState::Closed));
}

#[test]
fn stale_lost_and_refused_remain_distinct() {
    let endpoint = NetworkEndpoint {
        address: NetworkAddress::Ipv4(Ipv4Addr::LOCALHOST.octets()),
        port: 9,
        transport: NetworkTransport::Tcp,
    };
    assert!(matches!(
        connect_tcp(
            &endpoint,
            EndpointFreshness::Stale,
            NetworkProviderAvailability::Available,
            Duration::from_millis(10),
        )[0],
        NetworkConnectionState::StaleEndpoint { .. }
    ));
    assert!(matches!(
        connect_tcp(
            &endpoint,
            EndpointFreshness::Current,
            NetworkProviderAvailability::Lost,
            Duration::from_millis(10),
        )[0],
        NetworkConnectionState::Lost { .. }
    ));
    let unresolved = NetworkEndpoint {
        address: NetworkAddress::DnsName("localhost".to_string()),
        ..endpoint.clone()
    };
    assert!(matches!(
        connect_tcp(
            &unresolved,
            EndpointFreshness::Current,
            NetworkProviderAvailability::Available,
            Duration::from_millis(10),
        )[1],
        NetworkConnectionState::Refused { .. }
    ));
    assert!(matches!(
        resolve_dns_with_provider(
            &DnsQuery {
                name: "localhost".to_string(),
                port: 80,
                record_kind: DnsRecordKind::Address,
            },
            NetworkProviderAvailability::Lost,
        ),
        DnsResult::ProviderLost { .. }
    ));
}

#[test]
fn reviewed_frame_payload_is_inline_bounded_or_a_resource_reference() {
    let frame = NetworkProtocolFrame {
        protocol: NetworkFrameProtocol::EchoV1,
        direction: NetworkFrameDirection::Sent,
        sequence: 1,
        payload: NetworkFramePayload::Inline(vec![0; NETWORK_MAXIMUM_INLINE_PAYLOAD_BYTES + 1]),
    };
    assert!(frame.validate().is_err());
}
