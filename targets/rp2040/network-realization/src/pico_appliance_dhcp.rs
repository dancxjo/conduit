//! Minimal finite DHCPv4 server wire contract for the Pico Hello appliance.

use crate::{ApplianceFailure, DhcpLease, DhcpLeasePool};

pub const MAXIMUM_DHCP_PACKET_BYTES: usize = 576;
pub const DHCP_SERVER_ADDRESS: [u8; 4] = [192, 168, 4, 1];
pub const DHCP_SUBNET_MASK: [u8; 4] = [255, 255, 255, 0];
pub const DHCP_LEASE_SECONDS: u32 = 600;

const BOOTP_FIXED_BYTES: usize = 236;
const DHCP_OPTIONS_OFFSET: usize = 240;
const DHCP_MAGIC_COOKIE: [u8; 4] = [99, 130, 83, 99];
const OPTION_SUBNET_MASK: u8 = 1;
const OPTION_DNS: u8 = 6;
const OPTION_REQUESTED_ADDRESS: u8 = 50;
const OPTION_LEASE_TIME: u8 = 51;
const OPTION_MESSAGE_TYPE: u8 = 53;
const OPTION_SERVER_IDENTIFIER: u8 = 54;
const OPTION_END: u8 = 255;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DhcpRequestKind {
    Discover,
    Request,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DhcpResponseKind {
    Offer,
    Acknowledgement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DhcpResponse {
    pub len: usize,
    pub kind: DhcpResponseKind,
    pub lease: DhcpLease,
}

pub fn answer_appliance_dhcp(
    request: &[u8],
    leases: &mut DhcpLeasePool,
    output: &mut [u8],
) -> Result<DhcpResponse, ApplianceFailure> {
    if !leases.available() {
        return Err(ApplianceFailure::ServiceBaseLost(
            crate::ApplianceService::Dhcp,
        ));
    }
    if request.len() > MAXIMUM_DHCP_PACKET_BYTES {
        return Err(ApplianceFailure::OversizedDhcpRequest);
    }
    let parsed = parse_request(request)?;
    let lease = leases.lease(parsed.client)?;
    if parsed.kind == DhcpRequestKind::Request
        && parsed
            .requested_address
            .is_some_and(|requested| requested != lease.address)
    {
        return Err(ApplianceFailure::DhcpAddressMismatch);
    }
    if parsed
        .server_identifier
        .is_some_and(|server| server != DHCP_SERVER_ADDRESS)
    {
        return Err(ApplianceFailure::DhcpServerMismatch);
    }
    let kind = match parsed.kind {
        DhcpRequestKind::Discover => DhcpResponseKind::Offer,
        DhcpRequestKind::Request => DhcpResponseKind::Acknowledgement,
    };
    let len = encode_response(request, parsed, lease, kind, output)?;
    Ok(DhcpResponse { len, kind, lease })
}

#[derive(Clone, Copy)]
struct ParsedRequest {
    kind: DhcpRequestKind,
    client: [u8; 6],
    requested_address: Option<[u8; 4]>,
    server_identifier: Option<[u8; 4]>,
}

fn parse_request(request: &[u8]) -> Result<ParsedRequest, ApplianceFailure> {
    if request.len() < DHCP_OPTIONS_OFFSET + 4
        || request[0] != 1
        || request[1] != 1
        || request[2] != 6
        || request[3] != 0
        || request[BOOTP_FIXED_BYTES..DHCP_OPTIONS_OFFSET] != DHCP_MAGIC_COOKIE
    {
        return Err(ApplianceFailure::MalformedDhcpRequest);
    }
    let client: [u8; 6] = request[28..34]
        .try_into()
        .map_err(|_| ApplianceFailure::MalformedDhcpRequest)?;
    if client == [0; 6] {
        return Err(ApplianceFailure::MalformedDhcpRequest);
    }
    let mut message_type = None;
    let mut requested_address = None;
    let mut server_identifier = None;
    let mut cursor = DHCP_OPTIONS_OFFSET;
    while cursor < request.len() {
        let option = request[cursor];
        cursor += 1;
        match option {
            0 => continue,
            OPTION_END => break,
            _ => {
                let len = usize::from(
                    *request
                        .get(cursor)
                        .ok_or(ApplianceFailure::MalformedDhcpRequest)?,
                );
                cursor += 1;
                let end = cursor
                    .checked_add(len)
                    .filter(|end| *end <= request.len())
                    .ok_or(ApplianceFailure::MalformedDhcpRequest)?;
                match (option, len) {
                    (OPTION_MESSAGE_TYPE, 1) => message_type = Some(request[cursor]),
                    (OPTION_REQUESTED_ADDRESS, 4) => {
                        requested_address = Some(
                            request[cursor..end]
                                .try_into()
                                .map_err(|_| ApplianceFailure::MalformedDhcpRequest)?,
                        )
                    }
                    (OPTION_SERVER_IDENTIFIER, 4) => {
                        server_identifier = Some(
                            request[cursor..end]
                                .try_into()
                                .map_err(|_| ApplianceFailure::MalformedDhcpRequest)?,
                        )
                    }
                    _ => {}
                }
                cursor = end;
            }
        }
    }
    let kind = match message_type {
        Some(1) => DhcpRequestKind::Discover,
        Some(3) => DhcpRequestKind::Request,
        _ => return Err(ApplianceFailure::MalformedDhcpRequest),
    };
    Ok(ParsedRequest {
        kind,
        client,
        requested_address,
        server_identifier,
    })
}

fn encode_response(
    request: &[u8],
    parsed: ParsedRequest,
    lease: DhcpLease,
    kind: DhcpResponseKind,
    output: &mut [u8],
) -> Result<usize, ApplianceFailure> {
    // Keep the packet bounded by the admitted DHCP buffer, but transmit only
    // the bytes belonging to the encoded message. This matches the proven
    // Pico W AP implementation consumed by the same Embassy/smoltcp client.
    const MAXIMUM_RESPONSE_BYTES: usize = 300;
    if output.len() < MAXIMUM_RESPONSE_BYTES {
        return Err(ApplianceFailure::ResponseBufferTooSmall);
    }
    output[..MAXIMUM_RESPONSE_BYTES].fill(0);
    output[0..4].copy_from_slice(&[2, 1, 6, 0]);
    output[4..8].copy_from_slice(&request[4..8]);
    output[10..12].copy_from_slice(&request[10..12]);
    output[16..20].copy_from_slice(&lease.address);
    output[20..24].copy_from_slice(&DHCP_SERVER_ADDRESS);
    output[28..34].copy_from_slice(&parsed.client);
    output[BOOTP_FIXED_BYTES..DHCP_OPTIONS_OFFSET].copy_from_slice(&DHCP_MAGIC_COOKIE);
    let mut cursor = DHCP_OPTIONS_OFFSET;
    let response_type = match kind {
        DhcpResponseKind::Offer => 2,
        DhcpResponseKind::Acknowledgement => 5,
    };
    append_option(output, &mut cursor, OPTION_MESSAGE_TYPE, &[response_type]);
    append_option(
        output,
        &mut cursor,
        OPTION_SERVER_IDENTIFIER,
        &DHCP_SERVER_ADDRESS,
    );
    append_option(
        output,
        &mut cursor,
        OPTION_LEASE_TIME,
        &DHCP_LEASE_SECONDS.to_be_bytes(),
    );
    append_option(output, &mut cursor, OPTION_SUBNET_MASK, &DHCP_SUBNET_MASK);
    append_option(output, &mut cursor, OPTION_DNS, &DHCP_SERVER_ADDRESS);
    output[cursor] = OPTION_END;
    cursor += 1;
    debug_assert!(cursor <= MAXIMUM_RESPONSE_BYTES);
    Ok(cursor)
}

fn append_option(output: &mut [u8], cursor: &mut usize, option: u8, value: &[u8]) {
    output[*cursor] = option;
    output[*cursor + 1] = value.len() as u8;
    output[*cursor + 2..*cursor + 2 + value.len()].copy_from_slice(value);
    *cursor += value.len() + 2;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(kind: u8, requested: Option<[u8; 4]>) -> alloc::vec::Vec<u8> {
        let mut packet = alloc::vec![0; DHCP_OPTIONS_OFFSET];
        packet[0..4].copy_from_slice(&[1, 1, 6, 0]);
        packet[4..8].copy_from_slice(&0x1234_5678_u32.to_be_bytes());
        packet[10..12].copy_from_slice(&0x8000_u16.to_be_bytes());
        packet[28..34].copy_from_slice(&[2, 3, 4, 5, 6, 7]);
        packet[BOOTP_FIXED_BYTES..DHCP_OPTIONS_OFFSET].copy_from_slice(&DHCP_MAGIC_COOKIE);
        packet.extend_from_slice(&[OPTION_MESSAGE_TYPE, 1, kind]);
        if let Some(address) = requested {
            packet.extend_from_slice(&[OPTION_REQUESTED_ADDRESS, 4]);
            packet.extend_from_slice(&address);
            packet.extend_from_slice(&[OPTION_SERVER_IDENTIFIER, 4]);
            packet.extend_from_slice(&DHCP_SERVER_ADDRESS);
        }
        packet.push(OPTION_END);
        packet
    }

    #[test]
    fn discover_and_request_produce_stable_offer_then_ack() {
        let mut leases = DhcpLeasePool::default();
        let mut output = [0; MAXIMUM_DHCP_PACKET_BYTES];
        let offer = answer_appliance_dhcp(&request(1, None), &mut leases, &mut output).unwrap();
        assert_eq!(offer.kind, DhcpResponseKind::Offer);
        assert_eq!(offer.lease.address, [192, 168, 4, 2]);
        assert_eq!(&output[4..8], &0x1234_5678_u32.to_be_bytes());
        assert_eq!(&output[16..20], &[192, 168, 4, 2]);
        let ack = answer_appliance_dhcp(
            &request(3, Some(offer.lease.address)),
            &mut leases,
            &mut output,
        )
        .unwrap();
        assert_eq!(ack.kind, DhcpResponseKind::Acknowledgement);
        assert_eq!(ack.lease, offer.lease);
    }

    #[test]
    fn responses_parse_as_the_exact_embassy_client_wire_contract() {
        use smoltcp::wire::{DhcpMessageType, DhcpPacket, DhcpRepr, EthernetAddress};

        let mut leases = DhcpLeasePool::default();
        let mut output = [0; MAXIMUM_DHCP_PACKET_BYTES];
        for (request_kind, response_kind) in
            [(1, DhcpMessageType::Offer), (3, DhcpMessageType::Ack)]
        {
            let request = request(
                request_kind,
                (request_kind == 3).then_some([192, 168, 4, 2]),
            );
            let response = answer_appliance_dhcp(&request, &mut leases, &mut output).unwrap();
            let packet = DhcpPacket::new_checked(&output[..response.len]).unwrap();
            let parsed = DhcpRepr::parse(&packet).unwrap();
            assert_eq!(parsed.message_type, response_kind);
            assert_eq!(parsed.transaction_id, 0x1234_5678);
            assert_eq!(
                parsed.client_hardware_address,
                EthernetAddress([2, 3, 4, 5, 6, 7])
            );
            assert_eq!(parsed.your_ip.octets(), [192, 168, 4, 2]);
            assert_eq!(
                parsed.server_identifier.unwrap().octets(),
                DHCP_SERVER_ADDRESS
            );
            assert_eq!(parsed.subnet_mask.unwrap().octets(), DHCP_SUBNET_MASK);
            assert_eq!(parsed.router, None);
        }
    }

    #[test]
    fn malformed_oversized_and_wrong_address_remain_distinct() {
        let mut leases = DhcpLeasePool::default();
        let mut output = [0; MAXIMUM_DHCP_PACKET_BYTES];
        assert_eq!(
            answer_appliance_dhcp(&[0; 10], &mut leases, &mut output),
            Err(ApplianceFailure::MalformedDhcpRequest)
        );
        assert_eq!(
            answer_appliance_dhcp(
                &[0; MAXIMUM_DHCP_PACKET_BYTES + 1],
                &mut leases,
                &mut output
            ),
            Err(ApplianceFailure::OversizedDhcpRequest)
        );
        assert_eq!(
            answer_appliance_dhcp(
                &request(3, Some([192, 168, 4, 99])),
                &mut leases,
                &mut output
            ),
            Err(ApplianceFailure::DhcpAddressMismatch)
        );
    }
}
