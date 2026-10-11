//! Actual existing smoltcp queue Device; no USB/network service proof.
use super::*;
fn scope() -> EthernetNetworkScope {
    EthernetNetworkScope {
        mac: [2, 0, 0, 0, 0, 1],
        endpoint: VirtioTcpEndpoint {
            guest_address: [10, 0, 2, 15],
            prefix_length: 24,
            gateway: [10, 0, 2, 2],
            remote_address: [10, 0, 2, 2],
            remote_port: 8080,
            local_port: 40000,
        },
        maximum_polls: 1,
    }
}
fn approval(scope: EthernetNetworkScope) -> EthernetNetworkApproval {
    // SAFETY: test-only explicit Root approval; not native administrator proof.
    unsafe { EthernetNetworkApproval::admit("host/test".into(), "boot/test".into(), scope) }
        .unwrap()
}
#[test]
fn invalid_root_scope_cannot_prepare_approval() {
    for mutate in [0, 1, 2, 3, 4] {
        let mut scope = scope();
        match mutate {
            0 => scope.mac = [0; 6],
            1 => scope.mac[0] = 1,
            2 => scope.maximum_polls = 0,
            3 => scope.endpoint.remote_port = 0,
            _ => scope.endpoint.prefix_length = 33,
        }
        assert!(matches!(
            unsafe {
                EthernetNetworkApproval::admit("host/test".into(), "boot/test".into(), scope)
            },
            Err(EthernetNetworkRefusal::Approval)
        ));
    }
}
#[test]
fn bounded_poll_uses_same_queue_and_retains_sticky_tcp_timeout() {
    let scope = scope();
    let queues = EthernetFrameQueues::prepare(7, scope.mac).unwrap();
    let mut response = [0; 16];
    let mut rx = [0; 1024];
    let mut tx = [0; 1024];
    let mut sockets = [SocketStorage::EMPTY];
    let mut network = PreparedEthernetNetwork::prepare(
        &queues,
        approval(scope),
        EthernetNetworkStorage {
            request: b"request",
            response: &mut response,
            expected_response_bytes: 1,
            receive: &mut rx,
            transmit: &mut tx,
            sockets: &mut sockets,
        },
        7,
        Instant::ZERO,
    )
    .unwrap();
    assert_eq!(
        network.poll_one(Instant::ZERO),
        Err(EthernetNetworkRefusal::Tcp(VirtioTcpError::Timeout))
    );
    let mut wire = [0; super::super::queues::FRAME_BYTES];
    let (ordinal, length) = queues.take_transmit(7, &mut wire).unwrap();
    assert_eq!(ordinal, 0);
    assert_eq!(length, 42);
    assert_eq!(&wire[12..14], &[8, 6]);
    assert_eq!(&wire[6..12], &scope.mac);
    assert_eq!(
        network.poll_one(Instant::from_millis(1)),
        Err(EthernetNetworkRefusal::Tcp(VirtioTcpError::Timeout))
    );
    assert_eq!(network.polls(), 1);
}
#[test]
fn local_cancel_prevents_any_stack_transmit_and_cannot_create_network_permission() {
    let scope = scope();
    let queues = EthernetFrameQueues::prepare(7, scope.mac).unwrap();
    let mut response = [0; 16];
    let mut rx = [0; 1024];
    let mut tx = [0; 1024];
    let mut sockets = [SocketStorage::EMPTY];
    let mut network = PreparedEthernetNetwork::prepare(
        &queues,
        approval(scope),
        EthernetNetworkStorage {
            request: b"request",
            response: &mut response,
            expected_response_bytes: 1,
            receive: &mut rx,
            transmit: &mut tx,
            sockets: &mut sockets,
        },
        7,
        Instant::ZERO,
    )
    .unwrap();
    network.cancel();
    assert_eq!(
        network.poll_one(Instant::ZERO),
        Err(EthernetNetworkRefusal::Tcp(VirtioTcpError::Cancelled))
    );
    assert_eq!(network.polls(), 0);
    let mut wire = [0; super::super::queues::FRAME_BYTES];
    assert_eq!(
        queues.take_transmit(7, &mut wire),
        Err(FrameRefusal::Pressure)
    );
}
