//! Allocation-free bounded protocol state for the Pico Hello appliance Bases.

use crate::{
    APPLIANCE_HELLO_BODY, APPLIANCE_LOCAL_NAME, MAXIMUM_APPLIANCE_SIGNS, MAXIMUM_DHCP_LEASES,
    MAXIMUM_DNS_PACKET_BYTES, MAXIMUM_HTTP_REQUEST_BYTES,
};

const APPLIANCE_ADDRESS_PREFIX: [u8; 3] = [192, 168, 4];
const FIRST_CLIENT_ADDRESS: u8 = 2;
const DNS_HEADER_BYTES: usize = 12;
const DNS_TYPE_A: u16 = 1;
const DNS_CLASS_IN: u16 = 1;
pub const HTTP_HELLO_RESPONSE: &[u8] =
    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 19\r\nConnection: close\r\n\r\nHello from Conduit\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplianceService {
    AccessPoint,
    Dhcp,
    Dns,
    Http,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplianceFailure {
    MissingRadioArtifact,
    RadioInitializationFailed,
    DhcpPoolExhausted,
    MalformedDnsRequest,
    OversizedDnsRequest,
    MalformedHttpRequest,
    OversizedHttpRequest,
    ResponseBufferTooSmall,
    ServiceBaseLost(ApplianceService),
    SignCapacityExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplianceSignKind {
    AccessPointReady,
    ClientAssociated,
    DhcpLeaseGranted,
    DnsRequestReceived,
    DnsResponseSent,
    HttpRequestReceived,
    HttpResponseSent,
    Terminal,
    Failure(ApplianceFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplianceSign {
    pub sequence: u16,
    pub kind: ApplianceSignKind,
}

pub struct ApplianceSignLog<const N: usize = { MAXIMUM_APPLIANCE_SIGNS as usize }> {
    entries: [Option<ApplianceSign>; N],
    len: usize,
}

impl<const N: usize> Default for ApplianceSignLog<N> {
    fn default() -> Self {
        Self {
            entries: [None; N],
            len: 0,
        }
    }
}

impl<const N: usize> ApplianceSignLog<N> {
    pub fn push(&mut self, kind: ApplianceSignKind) -> Result<ApplianceSign, ApplianceFailure> {
        let slot = self
            .entries
            .get_mut(self.len)
            .ok_or(ApplianceFailure::SignCapacityExhausted)?;
        let sign = ApplianceSign {
            sequence: u16::try_from(self.len + 1)
                .map_err(|_| ApplianceFailure::SignCapacityExhausted)?,
            kind,
        };
        *slot = Some(sign);
        self.len += 1;
        Ok(sign)
    }

    pub fn as_slice(&self) -> &[Option<ApplianceSign>] {
        &self.entries[..self.len]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DhcpLease {
    pub client: [u8; 6],
    pub address: [u8; 4],
}

pub struct DhcpLeasePool {
    clients: [Option<[u8; 6]>; MAXIMUM_DHCP_LEASES as usize],
    available: bool,
}

impl Default for DhcpLeasePool {
    fn default() -> Self {
        Self {
            clients: [None; MAXIMUM_DHCP_LEASES as usize],
            available: true,
        }
    }
}

impl DhcpLeasePool {
    pub fn lose_base(&mut self) {
        self.available = false;
    }

    pub fn lease(&mut self, client: [u8; 6]) -> Result<DhcpLease, ApplianceFailure> {
        if !self.available {
            return Err(ApplianceFailure::ServiceBaseLost(ApplianceService::Dhcp));
        }
        if let Some(index) = self.clients.iter().position(|entry| *entry == Some(client)) {
            return Ok(lease_at(index, client));
        }
        let index = self
            .clients
            .iter()
            .position(Option::is_none)
            .ok_or(ApplianceFailure::DhcpPoolExhausted)?;
        self.clients[index] = Some(client);
        Ok(lease_at(index, client))
    }
}

fn lease_at(index: usize, client: [u8; 6]) -> DhcpLease {
    DhcpLease {
        client,
        address: [
            APPLIANCE_ADDRESS_PREFIX[0],
            APPLIANCE_ADDRESS_PREFIX[1],
            APPLIANCE_ADDRESS_PREFIX[2],
            FIRST_CLIENT_ADDRESS + index as u8,
        ],
    }
}

pub fn answer_appliance_dns(request: &[u8], output: &mut [u8]) -> Result<usize, ApplianceFailure> {
    if request.len() > MAXIMUM_DNS_PACKET_BYTES as usize {
        return Err(ApplianceFailure::OversizedDnsRequest);
    }
    let question_end = validate_dns_question(request)?;
    let response_len = question_end
        .checked_add(16)
        .ok_or(ApplianceFailure::MalformedDnsRequest)?;
    if output.len() < response_len {
        return Err(ApplianceFailure::ResponseBufferTooSmall);
    }
    output[..question_end].copy_from_slice(&request[..question_end]);
    output[2..4].copy_from_slice(&0x8180_u16.to_be_bytes());
    output[6..8].copy_from_slice(&1_u16.to_be_bytes());
    let mut cursor = question_end;
    output[cursor..cursor + 2].copy_from_slice(&[0xc0, 0x0c]);
    cursor += 2;
    output[cursor..cursor + 2].copy_from_slice(&DNS_TYPE_A.to_be_bytes());
    cursor += 2;
    output[cursor..cursor + 2].copy_from_slice(&DNS_CLASS_IN.to_be_bytes());
    cursor += 2;
    output[cursor..cursor + 4].copy_from_slice(&60_u32.to_be_bytes());
    cursor += 4;
    output[cursor..cursor + 2].copy_from_slice(&4_u16.to_be_bytes());
    cursor += 2;
    output[cursor..cursor + 4].copy_from_slice(&[192, 168, 4, 1]);
    Ok(response_len)
}

fn validate_dns_question(request: &[u8]) -> Result<usize, ApplianceFailure> {
    if request.len() < DNS_HEADER_BYTES + 5
        || request[2] & 0x80 != 0
        || u16::from_be_bytes([request[4], request[5]]) != 1
        || request[6..12] != [0; 6]
    {
        return Err(ApplianceFailure::MalformedDnsRequest);
    }
    let expected = APPLIANCE_LOCAL_NAME.as_bytes();
    let mut request_cursor = DNS_HEADER_BYTES;
    let mut expected_cursor = 0;
    while expected_cursor < expected.len() {
        let label_end = expected[expected_cursor..]
            .iter()
            .position(|byte| *byte == b'.')
            .map_or(expected.len(), |offset| expected_cursor + offset);
        let label = &expected[expected_cursor..label_end];
        if request.get(request_cursor).copied() != Some(label.len() as u8)
            || request.get(request_cursor + 1..request_cursor + 1 + label.len()) != Some(label)
        {
            return Err(ApplianceFailure::MalformedDnsRequest);
        }
        request_cursor += label.len() + 1;
        expected_cursor = label_end.saturating_add(1);
    }
    if request.get(request_cursor).copied() != Some(0) {
        return Err(ApplianceFailure::MalformedDnsRequest);
    }
    request_cursor += 1;
    let question_end = request_cursor
        .checked_add(4)
        .ok_or(ApplianceFailure::MalformedDnsRequest)?;
    if question_end != request.len()
        || u16::from_be_bytes([request[request_cursor], request[request_cursor + 1]]) != DNS_TYPE_A
        || u16::from_be_bytes([request[request_cursor + 2], request[request_cursor + 3]])
            != DNS_CLASS_IN
    {
        return Err(ApplianceFailure::MalformedDnsRequest);
    }
    Ok(question_end)
}

pub fn answer_appliance_http(request: &[u8], output: &mut [u8]) -> Result<usize, ApplianceFailure> {
    if request.len() > MAXIMUM_HTTP_REQUEST_BYTES as usize {
        return Err(ApplianceFailure::OversizedHttpRequest);
    }
    if !request.ends_with(b"\r\n\r\n")
        || !request.starts_with(b"GET / HTTP/1.1\r\n")
        || !request
            .windows(b"Host: hello.conduit\r\n".len())
            .any(|window| window == b"Host: hello.conduit\r\n")
    {
        return Err(ApplianceFailure::MalformedHttpRequest);
    }
    if output.len() < HTTP_HELLO_RESPONSE.len() {
        return Err(ApplianceFailure::ResponseBufferTooSmall);
    }
    output[..HTTP_HELLO_RESPONSE.len()].copy_from_slice(HTTP_HELLO_RESPONSE);
    debug_assert!(HTTP_HELLO_RESPONSE.ends_with(APPLIANCE_HELLO_BODY.as_bytes()));
    Ok(HTTP_HELLO_RESPONSE.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dns_query(name: &str) -> alloc::vec::Vec<u8> {
        let mut query = alloc::vec![0x12, 0x34, 0x01, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        for label in name.split('.') {
            query.push(label.len() as u8);
            query.extend_from_slice(label.as_bytes());
        }
        query.extend_from_slice(&[0, 0, 1, 0, 1]);
        query
    }

    #[test]
    fn lease_pool_is_finite_stable_and_reports_exhaustion() {
        let mut pool = DhcpLeasePool::default();
        for client_index in 0..MAXIMUM_DHCP_LEASES {
            let client = [0, 1, 2, 3, 4, client_index as u8];
            let lease = pool.lease(client).unwrap();
            assert_eq!(lease.address, [192, 168, 4, 2 + client_index as u8]);
            assert_eq!(pool.lease(client).unwrap(), lease);
        }
        assert_eq!(pool.lease([9; 6]), Err(ApplianceFailure::DhcpPoolExhausted));
        pool.lose_base();
        assert_eq!(
            pool.lease([0; 6]),
            Err(ApplianceFailure::ServiceBaseLost(ApplianceService::Dhcp))
        );
    }

    #[test]
    fn dns_answers_only_the_reviewed_bounded_a_query() {
        let query = dns_query(APPLIANCE_LOCAL_NAME);
        let mut response = [0; MAXIMUM_DNS_PACKET_BYTES as usize];
        let len = answer_appliance_dns(&query, &mut response).unwrap();
        assert_eq!(&response[..2], &[0x12, 0x34]);
        assert_eq!(&response[len - 4..len], &[192, 168, 4, 1]);
        assert_eq!(
            answer_appliance_dns(&dns_query("wrong.conduit"), &mut response),
            Err(ApplianceFailure::MalformedDnsRequest)
        );
        assert_eq!(
            answer_appliance_dns(&[0; MAXIMUM_DNS_PACKET_BYTES as usize + 1], &mut response),
            Err(ApplianceFailure::OversizedDnsRequest)
        );
    }

    #[test]
    fn http_answers_only_literal_reviewed_request_and_body() {
        let request = b"GET / HTTP/1.1\r\nHost: hello.conduit\r\nConnection: close\r\n\r\n";
        let mut response = [0; 128];
        let len = answer_appliance_http(request, &mut response).unwrap();
        assert_eq!(&response[..len], HTTP_HELLO_RESPONSE);
        assert!(response[..len].ends_with(APPLIANCE_HELLO_BODY.as_bytes()));
        assert_eq!(
            answer_appliance_http(
                b"GET /other HTTP/1.1\r\nHost: hello.conduit\r\n\r\n",
                &mut response
            ),
            Err(ApplianceFailure::MalformedHttpRequest)
        );
        assert_eq!(
            answer_appliance_http(
                &[b'x'; MAXIMUM_HTTP_REQUEST_BYTES as usize + 1],
                &mut response
            ),
            Err(ApplianceFailure::OversizedHttpRequest)
        );
    }

    #[test]
    fn signs_are_ordered_and_fail_closed_at_the_admitted_bound() {
        let mut signs = ApplianceSignLog::<2>::default();
        assert_eq!(
            signs
                .push(ApplianceSignKind::AccessPointReady)
                .unwrap()
                .sequence,
            1
        );
        assert_eq!(
            signs
                .push(ApplianceSignKind::ClientAssociated)
                .unwrap()
                .sequence,
            2
        );
        assert_eq!(
            signs.push(ApplianceSignKind::Terminal),
            Err(ApplianceFailure::SignCapacityExhausted)
        );
    }
}
