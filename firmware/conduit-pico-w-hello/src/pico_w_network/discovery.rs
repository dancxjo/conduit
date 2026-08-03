use crate::pico_w_network::dhcp::{DhcpGrant, DHCP_LEASE_SECONDS};

const AP_IP_OCTETS: [u8; 4] = [192, 168, 4, 1];
const DNS_QUESTION_OFFSET: usize = 12;
const MDNS_NAME: &[u8] = b"\x07conduit\x05local\x00";
const CONDUIT_INTERNAL: &[u8] = b"\x07conduit\x08internal\x00";
const HELLO_CONDUIT_INTERNAL: &[u8] = b"\x05hello\x07conduit\x08internal\x00";
const GATEWAY_CONDUIT_INTERNAL: &[u8] = b"\x07gateway\x07conduit\x08internal\x00";

#[allow(clippy::too_many_arguments)]
pub fn build_dhcp_reply<'a>(
    grant: DhcpGrant,
    request: &[u8],
    response: &'a mut [u8; 576],
) -> Option<&'a [u8]> {
    response.fill(0);
    response[0] = 2;
    response[1] = request[1];
    response[2] = request[2];
    response[3] = request[3];
    response[4..8].copy_from_slice(&request[4..8]);
    response[10..12].copy_from_slice(&request[10..12]);
    response[16..20].copy_from_slice(&grant.lease_ip());
    response[20..24].copy_from_slice(&AP_IP_OCTETS);
    response[28..44].copy_from_slice(&request[28..44]);
    response[236..240].copy_from_slice(&[99, 130, 83, 99]);

    let mut i = 240;
    i = write_dhcp_option(i, response, 53, &[grant.reply_message_type()])?;
    i = write_dhcp_option(i, response, 54, &AP_IP_OCTETS)?;
    i = write_dhcp_option(i, response, 51, &DHCP_LEASE_SECONDS.to_be_bytes())?;
    i = write_dhcp_option(i, response, 1, &[255, 255, 255, 0])?;
    i = write_dhcp_option(i, response, 3, &AP_IP_OCTETS)?;
    i = write_dhcp_option(i, response, 6, &AP_IP_OCTETS)?;
    response[i] = 255;
    Some(&response[..i + 1])
}

pub fn build_mdns_announcement(packet: &mut [u8; 768]) -> usize {
    packet.fill(0);
    packet[2..4].copy_from_slice(&[0x84, 0x00]);
    packet[6..8].copy_from_slice(&1u16.to_be_bytes());
    let mut cursor = 12;
    cursor = mdns_a(packet, cursor, MDNS_NAME, AP_IP_OCTETS).unwrap_or(12);
    cursor
}

fn mdns_a(packet: &mut [u8], mut cursor: usize, name: &[u8], ip: [u8; 4]) -> Option<usize> {
    cursor = put_bytes(packet, cursor, name)?;
    cursor = put_bytes(packet, cursor, &[0, 1, 0x80, 1, 0, 0, 0, 120, 0, 4])?;
    put_bytes(packet, cursor, &ip)
}

fn put_bytes(packet: &mut [u8], offset: usize, bytes: &[u8]) -> Option<usize> {
    let end = offset.checked_add(bytes.len())?;
    packet.get_mut(offset..end)?.copy_from_slice(bytes);
    Some(end)
}

pub fn build_dns_reply(
    query: &[u8],
    response: &mut [u8; 512],
    gateway_ip: [u8; 4],
    _now_ms: u32,
) -> Option<&[u8]> {
    let question = parse_dns_question(query)?;
    let answer_ip = dns_answer_ip(&query[DNS_QUESTION_OFFSET..question.name_end], gateway_ip)?;
    if !matches!(question.qtype, 1 | 255) || !matches!(question.qclass, 1 | 255) {
        return None;
    }

    response[..question.end].copy_from_slice(&query[..question.end]);
    response[2] = 0x84 | (query[2] & 0x01);
    response[4] = 0x00;
    response[5] = 0x01;
    response[6] = 0x00;
    response[8] = 0x00;
    response[9] = 0x00;
    response[10] = 0x00;
    response[11] = 0x00;

    if answer_ip.is_none() {
        response[3] = 0x03;
        response[7] = 0x00;
        return Some(&response[..question.end]);
    }

    response[3] = 0x00;
    response[7] = 0x01;

    let mut i = question.end;
    let answer = [
        0xc0,
        0x0c,
        0x00,
        0x01,
        0x00,
        0x01,
        0x00,
        0x00,
        0x00,
        0x3c,
        0x00,
        0x04,
        answer_ip[0],
        answer_ip[1],
        answer_ip[2],
        answer_ip[3],
    ];
    if i + answer.len() > response.len() {
        return None;
    }
    response[i..i + answer.len()].copy_from_slice(&answer);
    i += answer.len();
    Some(&response[..i])
}

struct DnsQuestion {
    name_end: usize,
    end: usize,
    qtype: u16,
    qclass: u16,
}

fn parse_dns_question(packet: &[u8]) -> Option<DnsQuestion> {
    if packet.len() < 17 || packet[2] & 0x80 != 0 {
        return None;
    }
    let question_count = u16::from_be_bytes([packet[4], packet[5]]);
    if question_count == 0 {
        return None;
    }

    let mut i = 12;
    loop {
        let len = *packet.get(i)? as usize;
        if len & 0xc0 != 0 {
            return None;
        }
        i += 1;
        if len == 0 {
            break;
        }
        i = i.checked_add(len)?;
        if i > packet.len() {
            return None;
        }
    }

    let name_end = i;
    let end = i.checked_add(4)?;
    if end > packet.len() {
        return None;
    }
    Some(DnsQuestion {
        name_end,
        end,
        qtype: u16::from_be_bytes([packet[i], packet[i + 1]]),
        qclass: u16::from_be_bytes([packet[i + 2], packet[i + 3]]),
    })
}

fn dns_answer_ip(name: &[u8], gateway: [u8; 4]) -> Option<[u8; 4]> {
    if dns_name_eq(name, MDNS_NAME)
        || dns_name_eq(name, CONDUIT_INTERNAL)
        || dns_name_eq(name, GATEWAY_CONDUIT_INTERNAL)
    {
        return Some(gateway);
    }
    if dns_name_eq(name, HELLO_CONDUIT_INTERNAL) {
        return Some(AP_IP_OCTETS);
    }
    None
}

fn write_dhcp_option(offset: usize, packet: &mut [u8], option: u8, value: &[u8]) -> Option<usize> {
    let end = offset.checked_add(2)?.checked_add(value.len())?;
    if end >= packet.len() || value.len() > u8::MAX as usize {
        return None;
    }
    packet[offset] = option;
    packet[offset + 1] = value.len() as u8;
    packet[offset + 2..end].copy_from_slice(value);
    Some(end)
}

fn dns_name_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(left, right)| dns_byte_eq(*left, *right))
}

fn dns_byte_eq(left: u8, right: u8) -> bool {
    if left.is_ascii_alphabetic() && right.is_ascii_alphabetic() {
        left.to_ascii_lowercase() == right.to_ascii_lowercase()
    } else {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_matching_is_case_insensitive() {
        assert!(dns_name_eq(b"\x03ABC\x00", b"\x03abc\x00"));
    }
}
