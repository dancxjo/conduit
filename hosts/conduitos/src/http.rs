//! Fixed-storage, literal-address HTTP/1.1 client boundary for selected Images.

mod offer;
pub use offer::*;

const TYPE_BYTES: usize = 4_096;

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
    request_type: [u8; TYPE_BYTES],
    request_type_len: usize,
    response_type: [u8; TYPE_BYTES],
    response_type_len: usize,
}

impl NativeHttpClient {
    pub fn prepare() -> Self {
        let request = conduit_web::http_request_type().canonical_bytes().unwrap();
        let response = conduit_web::http_response_type().canonical_bytes().unwrap();
        assert!(request.len() <= TYPE_BYTES && response.len() <= TYPE_BYTES);
        let mut value = Self {
            request: [0; REQUEST_BYTES],
            request_len: 0,
            response: [0; RESPONSE_BYTES],
            pending: None,
            generation: 1,
            signs: 0,
            request_type: [0; TYPE_BYTES],
            request_type_len: request.len(),
            response_type: [0; TYPE_BYTES],
            response_type_len: response.len(),
        };
        value.request_type[..request.len()].copy_from_slice(&request);
        value.response_type[..response.len()].copy_from_slice(&response);
        value
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
        let (transaction, len) = encode_wire_request(
            input,
            &self.request_type[..self.request_type_len],
            &mut self.request,
        )?;
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
            &self.response_type[..self.response_type_len],
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

fn encode_wire_request(
    input: &[u8],
    request_type: &[u8],
    output: &mut [u8],
) -> Result<(u64, usize), HttpClientFailure> {
    if input.len() > REQUEST_BYTES {
        return Err(HttpClientFailure::RequestOverflow);
    }
    let encoded = input
        .strip_prefix(request_type)
        .ok_or(HttpClientFailure::MalformedRequest)?;
    let mut cursor = Cursor::new(encoded);
    cursor.record(5)?;
    cursor.field("body")?;
    cursor.variant("inline")?;
    let body = cursor.leaf()?;
    cursor.field("headers")?;
    cursor.collection(conduit_web::HTTP_MAXIMUM_HEADERS)?;
    let mut headers = [Header {
        name: &[],
        value: &[],
    }; conduit_web::HTTP_MAXIMUM_HEADERS];
    let mut count = 0;
    let mut unused_seen = false;
    for _ in 0..conduit_web::HTTP_MAXIMUM_HEADERS {
        cursor.tag(3)?;
        let tag = cursor.bytes()?;
        match tag {
            b"header" if !unused_seen => {
                cursor.record(2)?;
                cursor.field("name")?;
                let name = cursor.leaf()?;
                cursor.field("value")?;
                let value = cursor.leaf()?;
                headers[count] = Header { name, value };
                count += 1;
            }
            b"unused" => {
                unused_seen = true;
                if !cursor.leaf()?.is_empty() {
                    return Err(HttpClientFailure::MalformedRequest);
                }
            }
            _ => return Err(HttpClientFailure::MalformedRequest),
        }
    }
    cursor.field("method")?;
    cursor.tag(3)?;
    let method = match cursor.bytes()? {
        b"get" => b"GET".as_slice(),
        b"head" => b"HEAD".as_slice(),
        b"post" => b"POST".as_slice(),
        b"put" => b"PUT".as_slice(),
        b"patch" => b"PATCH".as_slice(),
        b"delete" => b"DELETE".as_slice(),
        b"options" => b"OPTIONS".as_slice(),
        _ => return Err(HttpClientFailure::MalformedRequest),
    };
    if !cursor.leaf()?.is_empty() {
        return Err(HttpClientFailure::MalformedRequest);
    }
    cursor.field("target")?;
    cursor.record(3)?;
    cursor.field("authority")?;
    let authority = cursor.leaf()?;
    cursor.field("path_and_query")?;
    let target = cursor.leaf()?;
    cursor.field("scheme")?;
    let scheme = cursor.leaf()?;
    cursor.field("transaction_id")?;
    let transaction = u64::from_le_bytes(
        cursor
            .leaf()?
            .try_into()
            .map_err(|_| HttpClientFailure::MalformedRequest)?,
    );
    if !cursor.finished() {
        return Err(HttpClientFailure::MalformedRequest);
    }
    if scheme == b"https" {
        return Err(HttpClientFailure::TlsUnsupported);
    }
    if scheme != b"http" {
        return Err(HttpClientFailure::MalformedRequest);
    }
    if !literal_authority(authority) {
        return Err(HttpClientFailure::NameResolutionUnsupported);
    }
    if target.is_empty()
        || target[0] != b'/'
        || target.len() > conduit_web::HTTP_MAXIMUM_TARGET_BYTES
    {
        return Err(HttpClientFailure::MalformedRequest);
    }
    if count > conduit_web::HTTP_MAXIMUM_HEADERS {
        return Err(HttpClientFailure::RequestOverflow);
    }
    for header in &headers[..count] {
        let name = header.name;
        let value = header.value;
        if name.is_empty()
            || name.len() > conduit_web::HTTP_MAXIMUM_HEADER_NAME_BYTES
            || value.len() > conduit_web::HTTP_MAXIMUM_HEADER_VALUE_BYTES
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
    }
    if body.len() > conduit_web::HTTP_MAXIMUM_REQUEST_BODY_BYTES {
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
    response_type: &[u8],
    output: &mut [u8],
) -> Result<usize, HttpClientFailure> {
    let split = find(wire, b"\r\n\r\n").ok_or(HttpClientFailure::MalformedResponse)?;
    let head = &wire[..split];
    let body = &wire[split + 4..];
    if body.len() > conduit_web::HTTP_MAXIMUM_RESPONSE_BODY_BYTES {
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
    }; conduit_web::HTTP_MAXIMUM_HEADERS];
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
            || name.len() > conduit_web::HTTP_MAXIMUM_HEADER_NAME_BYTES
            || value.len() > conduit_web::HTTP_MAXIMUM_HEADER_VALUE_BYTES
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
    writer.put(response_type)?;
    writer.record(4)?;
    writer.field("body")?;
    writer.variant("inline")?;
    writer.leaf(body)?;
    writer.field("headers")?;
    writer.collection(conduit_web::HTTP_MAXIMUM_HEADERS)?;
    for header in &headers[..count] {
        writer.variant("header")?;
        writer.record(2)?;
        writer.field("name")?;
        writer.leaf(header.name)?;
        writer.field("value")?;
        writer.leaf(header.value)?;
    }
    for _ in count..conduit_web::HTTP_MAXIMUM_HEADERS {
        writer.variant("unused")?;
        writer.leaf(&[])?;
    }
    writer.field("status")?;
    writer.leaf(&u64::from(status).to_le_bytes())?;
    writer.field("transaction_id")?;
    writer.leaf(&transaction.to_le_bytes())?;
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
    fn u32(&mut self) -> Result<u32, HttpClientFailure> {
        Ok(u32::from_le_bytes(
            self.take(4)?
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
    fn tag(&mut self, expected: u8) -> Result<(), HttpClientFailure> {
        if self.byte()? == expected {
            Ok(())
        } else {
            Err(HttpClientFailure::MalformedRequest)
        }
    }
    fn record(&mut self, fields: usize) -> Result<(), HttpClientFailure> {
        self.tag(2)?;
        if self.u32()? as usize == fields {
            Ok(())
        } else {
            Err(HttpClientFailure::MalformedRequest)
        }
    }
    fn collection(&mut self, items: usize) -> Result<(), HttpClientFailure> {
        self.tag(1)?;
        if self.u32()? as usize == items {
            Ok(())
        } else {
            Err(HttpClientFailure::MalformedRequest)
        }
    }
    fn field(&mut self, expected: &str) -> Result<(), HttpClientFailure> {
        if self.bytes()? == expected.as_bytes() {
            Ok(())
        } else {
            Err(HttpClientFailure::MalformedRequest)
        }
    }
    fn variant(&mut self, expected: &str) -> Result<(), HttpClientFailure> {
        self.tag(3)?;
        self.field(expected)
    }
    fn leaf(&mut self) -> Result<&'a [u8], HttpClientFailure> {
        self.tag(0)?;
        self.bytes()
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
        self.put(&len.to_le_bytes())
            .map_err(|_| HttpClientFailure::ResponseBodyOverflow)?;
        self.put(value)
            .map_err(|_| HttpClientFailure::ResponseBodyOverflow)
    }
    fn record(&mut self, fields: usize) -> Result<(), HttpClientFailure> {
        self.byte(2)?;
        self.put(&(fields as u32).to_le_bytes())
    }
    fn collection(&mut self, items: usize) -> Result<(), HttpClientFailure> {
        self.byte(1)?;
        self.put(&(items as u32).to_le_bytes())
    }
    fn field(&mut self, name: &str) -> Result<(), HttpClientFailure> {
        self.sized(name.as_bytes())
    }
    fn variant(&mut self, tag: &str) -> Result<(), HttpClientFailure> {
        self.byte(3)?;
        self.sized(tag.as_bytes())
    }
    fn leaf(&mut self, value: &[u8]) -> Result<(), HttpClientFailure> {
        self.byte(0)?;
        self.sized(value)
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
