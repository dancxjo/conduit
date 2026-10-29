//! The bounded direct-AP subset of RFC2131: selecting, renewing and rebinding.
//! No relay, lease expiry or physical transport success is claimed.
use conduit_rp2040_network_realization::*;
const CLIENT: [u8; 6] = [2, 3, 4, 5, 6, 7];
fn request(kind: u8, client: [u8; 6], ciaddr: [u8; 4], broadcast: bool, xid: u32) -> Vec<u8> {
    let mut p = vec![0; 240];
    p[..4].copy_from_slice(&[1, 1, 6, 0]);
    p[4..8].copy_from_slice(&xid.to_be_bytes());
    p[10..12].copy_from_slice(&(if broadcast { 0x8000u16 } else { 0 }).to_be_bytes());
    p[12..16].copy_from_slice(&ciaddr);
    p[28..34].copy_from_slice(&client);
    p[236..240].copy_from_slice(&[99, 130, 83, 99]);
    p.extend_from_slice(&[53, 1, kind, 255]);
    p
}
fn selecting(address: [u8; 4]) -> Vec<u8> {
    let mut p = request(3, CLIENT, [0; 4], true, 9);
    p.pop();
    p.extend_from_slice(&[50, 4]);
    p.extend_from_slice(&address);
    p.extend_from_slice(&[54, 4]);
    p.extend_from_slice(&DHCP_SERVER_ADDRESS);
    p.push(255);
    p
}
#[test]
fn original_discover_and_selecting_stay_broadcast_with_exact_nonce_and_identity() {
    let mut leases = DhcpLeasePool::default();
    let mut out = [0; MAXIMUM_DHCP_PACKET_BYTES];
    let offer =
        answer_appliance_dhcp(&request(1, CLIENT, [0; 4], true, 8), &mut leases, &mut out).unwrap();
    assert_eq!(offer.destination, [255; 4]);
    assert_eq!(offer.kind, DhcpResponseKind::Offer);
    let ack =
        answer_appliance_dhcp(&selecting(offer.lease.address), &mut leases, &mut out).unwrap();
    assert_eq!(ack.destination, [255; 4]);
    assert_eq!(ack.lease, offer.lease);
    assert_eq!(ack.kind, DhcpResponseKind::Acknowledgement);
    assert_eq!(&out[4..8], &9u32.to_be_bytes());
    assert_eq!(&out[12..16], &[0; 4]);
    assert_eq!(&out[28..34], &CLIENT);
}
#[test]
fn renew_and_rebind_ack_the_retained_client_address_without_selecting_options() {
    let mut leases = DhcpLeasePool::default();
    let lease = leases.lease(CLIENT).unwrap();
    let mut out = [0; MAXIMUM_DHCP_PACKET_BYTES];
    for (broadcast, xid) in [(false, 0x12345678), (true, 0x87654321)] {
        // Both RENEWING and REBINDING carry ciaddr and omit options50/54.
        let packet = request(3, CLIENT, lease.address, broadcast, xid);
        let ack = answer_appliance_dhcp(&packet, &mut leases, &mut out).unwrap();
        assert_eq!(ack.destination, lease.address);
        assert_eq!(ack.kind, DhcpResponseKind::Acknowledgement);
        assert_eq!(ack.lease, lease);
        assert_eq!(&out[4..8], &xid.to_be_bytes());
        assert_eq!(&out[10..12], &packet[10..12]);
        assert_eq!(&out[12..16], &lease.address);
        assert_eq!(&out[16..20], &lease.address);
        assert_eq!(&out[28..34], &CLIENT);
        assert!(ack.len <= 300);
        assert_eq!(MAXIMUM_DHCP_PACKET_BYTES, 576);
        use smoltcp::wire::{DhcpMessageType, DhcpPacket, DhcpRepr};
        let wire = DhcpPacket::new_checked(&out[..ack.len]).unwrap();
        let parsed = DhcpRepr::parse(&wire).unwrap();
        assert_eq!(parsed.message_type, DhcpMessageType::Ack);
        assert_eq!(parsed.client_ip.octets(), lease.address);
        assert_eq!(parsed.transaction_id, xid);
        assert_eq!(parsed.lease_duration, Some(600));
    }
}
#[test]
fn unknown_or_mismatched_renewal_cannot_consume_the_four_admitted_slots() {
    let mut leases = DhcpLeasePool::default();
    let lease = leases.lease(CLIENT).unwrap();
    let mut out = [0; MAXIMUM_DHCP_PACKET_BYTES];
    for client in [[8; 6], CLIENT] {
        let wrong = if client == CLIENT {
            [192, 168, 4, 99]
        } else {
            lease.address
        };
        assert_eq!(
            answer_appliance_dhcp(&request(3, client, wrong, false, 11), &mut leases, &mut out),
            Err(ApplianceFailure::DhcpAddressMismatch)
        );
    }
    assert_eq!(leases.lease(CLIENT).unwrap(), lease);
    for n in 1..4 {
        leases.lease([n; 6]).unwrap();
    }
    assert_eq!(
        leases.lease([9; 6]),
        Err(ApplianceFailure::DhcpPoolExhausted)
    );
}
#[test]
fn unsupported_relay_and_foreign_server_refuse_before_any_slot_is_allocated() {
    let mut leases = DhcpLeasePool::default();
    let mut out = [0; MAXIMUM_DHCP_PACKET_BYTES];
    let mut relay = request(1, CLIENT, [0; 4], true, 13);
    relay[24..28].copy_from_slice(&[192, 168, 4, 254]);
    assert_eq!(
        answer_appliance_dhcp(&relay, &mut leases, &mut out),
        Err(ApplianceFailure::MalformedDhcpRequest)
    );
    let mut foreign = selecting([192, 168, 4, 2]);
    let end = foreign.len();
    foreign[end - 5..end - 1].copy_from_slice(&[192, 168, 4, 99]);
    assert_eq!(
        answer_appliance_dhcp(&foreign, &mut leases, &mut out),
        Err(ApplianceFailure::DhcpServerMismatch)
    );
    for n in 1..=4 {
        leases.lease([n; 6]).unwrap();
    }
}
#[test]
fn non_request_ciaddr_and_wrong_requested_address_do_not_become_a_renewal_ack() {
    let mut leases = DhcpLeasePool::default();
    let lease = leases.lease(CLIENT).unwrap();
    let mut out = [0; MAXIMUM_DHCP_PACKET_BYTES];
    assert_eq!(
        answer_appliance_dhcp(
            &request(1, CLIENT, lease.address, false, 17),
            &mut leases,
            &mut out
        ),
        Err(ApplianceFailure::MalformedDhcpRequest)
    );
    let mut packet = request(3, CLIENT, lease.address, false, 18);
    packet.pop();
    packet.extend_from_slice(&[50, 4, 192, 168, 4, 99, 255]);
    assert_eq!(
        answer_appliance_dhcp(&packet, &mut leases, &mut out),
        Err(ApplianceFailure::DhcpAddressMismatch)
    );
    assert_eq!(leases.lease(CLIENT).unwrap(), lease);
}

#[test]
fn withdrawn_service_cannot_ack_or_mutate_a_retained_client_renewal() {
    let mut leases = DhcpLeasePool::default();
    let lease = leases.lease(CLIENT).unwrap();
    leases.lose_base();
    let mut out = [0xa5; MAXIMUM_DHCP_PACKET_BYTES];
    let before = out;
    let refused = ApplianceFailure::ServiceBaseLost(ApplianceService::Dhcp);
    assert_eq!(
        answer_appliance_dhcp(
            &request(3, CLIENT, lease.address, false, 23),
            &mut leases,
            &mut out
        ),
        Err(refused)
    );
    assert_eq!(out, before);
    assert_eq!(leases.lease(CLIENT), Err(refused));
    assert_eq!(leases.lease([9; 6]), Err(refused));
}
