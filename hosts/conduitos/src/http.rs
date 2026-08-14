//! Fixed-storage, literal-address HTTP/1.1 client boundary for selected Images.

use conduit_core::{
    ArtifactId, AuthorityContractId, AuthorityRequirement, CapabilityId, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, kind_id, resource_requirement,
};

pub const IMPLEMENTATION: &str = "conduitos/kernel-http-client-http1-literal@1";
pub const PROFILE: &str = "conduitos/http1-literal-plain-fixed@1";
pub const ARTIFACT: &str = "conduitos/native-http1-fixed@1";
pub const HOST_OPERATION: &str = "conduit.host/http-client-exchange@1";
pub const RESOURCE_CLASS: &str = "conduit.resource/network/http-client@1";
pub const AUTHORITY: &str = "conduit.authority/http-outbound@1";
pub const NETWORK_BASE: &str = "network/ipv4-tcp";
pub const NETWORK_DRIVER: &str = "conduitos/deterministic-ipv4-tcp@1";
pub const FACILITY: &str = "network/http1-literal-client@1";
pub const PACKET_BUFFERS: u16 = 4;
pub const SOCKET_SLOTS: u16 = 1;
pub const TIMER_SLOTS: u16 = 2;
pub const SIGN_ITEMS: u16 = 32;
pub const REQUEST_BYTES: usize = conduit_std_catalog::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES as usize;
pub const RESPONSE_BYTES: usize = conduit_std_catalog::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES as usize;

pub fn offer() -> CapabilityOffer {
    let contract = conduit_std_catalog::http_client_contract();
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(HOST_OPERATION),
        target_kind: Some(kind_id(conduit_std_catalog::HTTP_REQUEST_INFO_ID)),
        maximum_in_flight: 1,
        maximum_input_bytes: REQUEST_BYTES as u32,
        maximum_output_bytes: RESPONSE_BYTES as u32,
    };
    CapabilityOffer {
        startup_parameters: alloc::vec::Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-http-client-http1-literal"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_std_catalog::HTTP_CLIENT_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        host_operations: alloc::vec![operation.clone()],
        resource_requirements: alloc::vec![resource_requirement(RESOURCE_CLASS, 1)],
        authority_requirements: alloc::vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(AUTHORITY),
            host_operation_contract_id: operation.contract_id,
            subject_kind: kind_id(conduit_std_catalog::HTTP_REQUEST_INFO_ID),
        }],
        limits: contract.limits,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum HttpClientFailure {
    MalformedRequest = 1,
    NameResolutionUnsupported = 2,
    TlsUnsupported = 3,
    Connect = 4,
    BaseLost = 5,
    ProviderLost = 6,
    Pressure = 7,
    Cancelled = 8,
    RequestOverflow = 9,
    ResponseHeaderOverflow = 10,
    ResponseBodyOverflow = 11,
    MalformedResponse = 12,
    StaleCompletion = 13,
    AuthorityDenied = 14,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkFailure {
    Connect,
    BaseLost,
    ProviderLost,
}

pub trait HttpNetworkBase {
    fn exchange(&mut self, request: &[u8], response: &mut [u8]) -> Result<usize, NetworkFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpTicket {
    generation: u32,
    transaction: u64,
}

pub struct FixedHttpOutput {
    bytes: [u8; RESPONSE_BYTES],
    len: usize,
}

impl FixedHttpOutput {
    pub const fn new() -> Self {
        Self {
            bytes: [0; RESPONSE_BYTES],
            len: 0,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl Default for FixedHttpOutput {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NativeHttpClient {
    request: [u8; REQUEST_BYTES],
    request_len: usize,
    response: [u8; RESPONSE_BYTES],
    pending: Option<HttpTicket>,
    generation: u32,
    signs: u16,
}

impl NativeHttpClient {
    pub const fn prepare() -> Self {
        Self {
            request: [0; REQUEST_BYTES],
            request_len: 0,
            response: [0; RESPONSE_BYTES],
            pending: None,
            generation: 1,
            signs: 0,
        }
    }

    pub fn begin(
        &mut self,
        input: &[u8],
        authorized: bool,
    ) -> Result<HttpTicket, HttpClientFailure> {
        if !authorized {
            return Err(HttpClientFailure::AuthorityDenied);
        }
        if self.pending.is_some() {
            return Err(HttpClientFailure::Pressure);
        }
        let (transaction, len) = encode_wire_request(input, &mut self.request)?;
        self.request_len = len;
        let ticket = HttpTicket {
            generation: self.generation,
            transaction,
        };
        self.pending = Some(ticket);
        self.signs = self.signs.saturating_add(1);
        Ok(ticket)
    }

    pub fn request_bytes(&self, ticket: HttpTicket) -> Result<&[u8], HttpClientFailure> {
        if self.pending != Some(ticket) {
            return Err(HttpClientFailure::StaleCompletion);
        }
        Ok(&self.request[..self.request_len])
    }

    pub fn complete(
        &mut self,
        ticket: HttpTicket,
        response_len: usize,
        output: &mut FixedHttpOutput,
    ) -> Result<(), HttpClientFailure> {
        if self.pending != Some(ticket) || ticket.generation != self.generation {
            return Err(HttpClientFailure::StaleCompletion);
        }
        if response_len > self.response.len() {
            return Err(HttpClientFailure::ResponseBodyOverflow);
        }
        let len = encode_info_response(
            ticket.transaction,
            &self.response[..response_len],
            &mut output.bytes,
        )?;
        output.len = len;
        self.pending = None;
        self.signs = self.signs.saturating_add(2);
        Ok(())
    }

    pub fn exchange<B: HttpNetworkBase>(
        &mut self,
        input: &[u8],
        authorized: bool,
        base: &mut B,
        output: &mut FixedHttpOutput,
    ) -> Result<(), HttpClientFailure> {
        let ticket = self.begin(input, authorized)?;
        let request = &self.request[..self.request_len];
        let response_len = base
            .exchange(request, &mut self.response)
            .map_err(map_network)?;
        self.complete(ticket, response_len, output)
    }

    pub fn cancel(&mut self) -> Result<(), HttpClientFailure> {
        if self.pending.take().is_none() {
            return Err(HttpClientFailure::Cancelled);
        }
        self.generation = self.generation.wrapping_add(1).max(1);
        self.signs = self.signs.saturating_add(1);
        Ok(())
    }

    pub const fn sign_count(&self) -> u16 {
        self.signs
    }
}

impl Default for NativeHttpClient {
    fn default() -> Self {
        Self::prepare()
    }
}

fn map_network(value: NetworkFailure) -> HttpClientFailure {
    match value {
        NetworkFailure::Connect => HttpClientFailure::Connect,
        NetworkFailure::BaseLost => HttpClientFailure::BaseLost,
        NetworkFailure::ProviderLost => HttpClientFailure::ProviderLost,
    }
}

#[derive(Clone, Copy)]
struct Header<'a> {
    name: &'a [u8],
    value: &'a [u8],
}

fn encode_wire_request(input: &[u8], output: &mut [u8]) -> Result<(u64, usize), HttpClientFailure> {
    if input.len() > REQUEST_BYTES {
        return Err(HttpClientFailure::RequestOverflow);
    }
    let mut cursor = Cursor::new(input);
    if cursor.byte()? != 1 {
        return Err(HttpClientFailure::MalformedRequest);
    }
    let transaction = cursor.u64()?;
    let method = match cursor.byte()? {
        0 => b"GET".as_slice(),
        1 => b"HEAD".as_slice(),
        2 => b"POST".as_slice(),
        3 => b"PUT".as_slice(),
        4 => b"PATCH".as_slice(),
        5 => b"DELETE".as_slice(),
        6 => b"OPTIONS".as_slice(),
        _ => return Err(HttpClientFailure::MalformedRequest),
    };
    let scheme = cursor.bytes()?;
    if scheme == b"https" {
        return Err(HttpClientFailure::TlsUnsupported);
    }
    if scheme != b"http" {
        return Err(HttpClientFailure::MalformedRequest);
    }
    let authority = cursor.bytes()?;
    if !literal_authority(authority) {
        return Err(HttpClientFailure::NameResolutionUnsupported);
    }
    let target = cursor.bytes()?;
    if target.is_empty()
        || target[0] != b'/'
        || target.len() > conduit_std_catalog::HTTP_MAXIMUM_TARGET_BYTES
    {
        return Err(HttpClientFailure::MalformedRequest);
    }
    let count = usize::from(cursor.u16()?);
    if count > conduit_std_catalog::HTTP_MAXIMUM_HEADERS {
        return Err(HttpClientFailure::RequestOverflow);
    }
    let mut headers = [Header {
        name: &[],
        value: &[],
    }; conduit_std_catalog::HTTP_MAXIMUM_HEADERS];
    for slot in headers.iter_mut().take(count) {
        let name = cursor.bytes()?;
        let value = cursor.bytes()?;
        if name.is_empty()
            || name.len() > conduit_std_catalog::HTTP_MAXIMUM_HEADER_NAME_BYTES
            || value.len() > conduit_std_catalog::HTTP_MAXIMUM_HEADER_VALUE_BYTES
            || !name.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || b"!#$%&'*+-.^_`|~".contains(byte)
            })
            || value.iter().any(|byte| matches!(byte, b'\r' | b'\n'))
            || matches!(
                name,
                b"authorization"
                    | b"proxy-authorization"
                    | b"cookie"
                    | b"set-cookie"
                    | b"content-length"
                    | b"transfer-encoding"
            )
        {
            return Err(HttpClientFailure::MalformedRequest);
        }
        *slot = Header { name, value };
    }
    let body = cursor.bytes()?;
    if body.len() > conduit_std_catalog::HTTP_MAXIMUM_REQUEST_BODY_BYTES || !cursor.finished() {
        return Err(HttpClientFailure::RequestOverflow);
    }
    let mut writer = Writer::new(output);
    writer.put(method)?;
    writer.put(b" ")?;
    writer.put(target)?;
    writer.put(b" HTTP/1.1\r\nHost: ")?;
    writer.put(authority)?;
    writer.put(b"\r\n")?;
    for header in &headers[..count] {
        writer.put(header.name)?;
        writer.put(b": ")?;
        writer.put(header.value)?;
        writer.put(b"\r\n")?;
    }
    writer.put(b"Content-Length: ")?;
    writer.decimal(body.len())?;
    writer.put(b"\r\nConnection: close\r\n\r\n")?;
    writer.put(body)?;
    Ok((transaction, writer.len))
}

fn literal_authority(value: &[u8]) -> bool {
    let Some(colon) = value.iter().rposition(|byte| *byte == b':') else {
        return false;
    };
    let (address, port) = (&value[..colon], &value[colon + 1..]);
    !address.is_empty()
        && !port.is_empty()
        && port.iter().all(u8::is_ascii_digit)
        && address
            .iter()
            .all(|byte| byte.is_ascii_digit() || *byte == b'.')
        && address.iter().filter(|byte| **byte == b'.').count() == 3
}

fn encode_info_response(
    transaction: u64,
    wire: &[u8],
    output: &mut [u8],
) -> Result<usize, HttpClientFailure> {
    let split = find(wire, b"\r\n\r\n").ok_or(HttpClientFailure::MalformedResponse)?;
    let head = &wire[..split];
    let body = &wire[split + 4..];
    if body.len() > conduit_std_catalog::HTTP_MAXIMUM_RESPONSE_BODY_BYTES {
        return Err(HttpClientFailure::ResponseBodyOverflow);
    }
    let first_end = find(head, b"\r\n").unwrap_or(head.len());
    let status_line = &head[..first_end];
    if status_line.len() < 12 || &status_line[..9] != b"HTTP/1.1 " {
        return Err(HttpClientFailure::MalformedResponse);
    }
    let status = decimal(&status_line[9..12]).ok_or(HttpClientFailure::MalformedResponse)?;
    if !(100..=599).contains(&status) {
        return Err(HttpClientFailure::MalformedResponse);
    }
    let mut headers = [Header {
        name: &[],
        value: &[],
    }; conduit_std_catalog::HTTP_MAXIMUM_HEADERS];
    let mut count = 0;
    let mut offset = first_end.saturating_add(2);
    let mut content_length = None;
    while offset < head.len() {
        if count == headers.len() {
            return Err(HttpClientFailure::ResponseHeaderOverflow);
        }
        let relative = find(&head[offset..], b"\r\n").unwrap_or(head.len() - offset);
        let line = &head[offset..offset + relative];
        offset += relative + 2;
        let colon = line
            .iter()
            .position(|byte| *byte == b':')
            .ok_or(HttpClientFailure::MalformedResponse)?;
        let name = &line[..colon];
        let value = line[colon + 1..]
            .strip_prefix(b" ")
            .unwrap_or(&line[colon + 1..]);
        if name.is_empty()
            || name.len() > conduit_std_catalog::HTTP_MAXIMUM_HEADER_NAME_BYTES
            || value.len() > conduit_std_catalog::HTTP_MAXIMUM_HEADER_VALUE_BYTES
            || !name
                .iter()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        {
            return Err(HttpClientFailure::ResponseHeaderOverflow);
        }
        if name == b"content-length" {
            content_length = decimal(value).map(usize::from);
            continue;
        }
        headers[count] = Header { name, value };
        count += 1;
    }
    if content_length != Some(body.len()) {
        return Err(HttpClientFailure::ProviderLost);
    }
    let mut writer = Writer::new(output);
    writer.byte(2)?;
    writer.put(&transaction.to_be_bytes())?;
    writer.put(&status.to_be_bytes())?;
    writer.put(&(count as u16).to_be_bytes())?;
    for header in &headers[..count] {
        writer.sized(header.name)?;
        writer.sized(header.value)?;
    }
    writer.sized(body)?;
    Ok(writer.len)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn decimal(input: &[u8]) -> Option<u16> {
    input.iter().try_fold(0_u16, |value, byte| {
        byte.is_ascii_digit().then_some(())?;
        value.checked_mul(10)?.checked_add(u16::from(*byte - b'0'))
    })
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], HttpClientFailure> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(HttpClientFailure::MalformedRequest)?;
        let value = self
            .input
            .get(self.offset..end)
            .ok_or(HttpClientFailure::MalformedRequest)?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, HttpClientFailure> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, HttpClientFailure> {
        Ok(u16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| HttpClientFailure::MalformedRequest)?,
        ))
    }
    fn u32(&mut self) -> Result<u32, HttpClientFailure> {
        Ok(u32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| HttpClientFailure::MalformedRequest)?,
        ))
    }
    fn u64(&mut self) -> Result<u64, HttpClientFailure> {
        Ok(u64::from_be_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| HttpClientFailure::MalformedRequest)?,
        ))
    }
    fn bytes(&mut self) -> Result<&'a [u8], HttpClientFailure> {
        let len = usize::try_from(self.u32()?).map_err(|_| HttpClientFailure::MalformedRequest)?;
        self.take(len)
    }
    fn finished(&self) -> bool {
        self.offset == self.input.len()
    }
}

struct Writer<'a> {
    output: &'a mut [u8],
    len: usize,
}

#[cfg(test)]
mod kernel_tests;
#[cfg(test)]
mod tests;

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self { output, len: 0 }
    }
    fn put(&mut self, value: &[u8]) -> Result<(), HttpClientFailure> {
        let end = self
            .len
            .checked_add(value.len())
            .ok_or(HttpClientFailure::RequestOverflow)?;
        self.output
            .get_mut(self.len..end)
            .ok_or(HttpClientFailure::RequestOverflow)?
            .copy_from_slice(value);
        self.len = end;
        Ok(())
    }
    fn byte(&mut self, value: u8) -> Result<(), HttpClientFailure> {
        self.put(&[value])
    }
    fn sized(&mut self, value: &[u8]) -> Result<(), HttpClientFailure> {
        let len =
            u32::try_from(value.len()).map_err(|_| HttpClientFailure::ResponseBodyOverflow)?;
        self.put(&len.to_be_bytes())
            .map_err(|_| HttpClientFailure::ResponseBodyOverflow)?;
        self.put(value)
            .map_err(|_| HttpClientFailure::ResponseBodyOverflow)
    }
    fn decimal(&mut self, mut value: usize) -> Result<(), HttpClientFailure> {
        let mut digits = [0_u8; 20];
        let mut index = digits.len();
        loop {
            index -= 1;
            digits[index] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        self.put(&digits[index..])
    }
}
