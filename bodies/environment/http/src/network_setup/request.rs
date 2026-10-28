//! Finite, single-request HTTP framing. Root still owns the connection deadline,
//! response flush, mailbox acknowledgement and admitted network Host Call.
use super::{NetworkSetupInput, NetworkSetupLimits, NetworkSetupRefusal};
use zeroize::Zeroize;

const REQUEST_BYTES: usize = 1024;
// Headers share the already admitted request buffer. A separate 512-byte cap
// refused ordinary browser requests despite sufficient total request capacity.
const HEADER_BYTES: usize = REQUEST_BYTES;

pub enum NetworkHttpRequest {
    Page,
    NetworkSnapshot(alloc::boxed::Box<crate::network_custody::Request>),
    ObservedOffer {
        slot: u8,
    },
    ObservedAck {
        slot: u8,
        bytes: alloc::vec::Vec<u8>,
    },
    BodyJoinPage,
    BodyJoinOptions,
    BodyJoin(alloc::vec::Vec<u8>),
    BodyJoinResult {
        ticket: u32,
    },
    Reading {
        json: bool,
    },
    History {
        after: u32,
    },
    Join(NetworkSetupInput),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkHttpRefusal {
    Closed,
    TooLarge,
    Malformed,
    UnsupportedRoute,
    UnsupportedContentType,
    Form(NetworkSetupRefusal),
}
/// Owns and erases raw credential bytes, including on cancellation. One instance
/// accepts one request; pipelining and chunked transfer are deliberately refused.
/// No Debug/Clone/serialization API exposes this secret-bearing storage.
pub struct NetworkRequestBuffer {
    bytes: [u8; REQUEST_BYTES],
    used: usize,
    closed: bool,
    readings: bool,
}
impl Default for NetworkRequestBuffer {
    fn default() -> Self {
        Self {
            bytes: [0; REQUEST_BYTES],
            used: 0,
            closed: false,
            readings: false,
        }
    }
}
impl NetworkRequestBuffer {
    pub(crate) fn for_http() -> Self {
        Self {
            readings: true,
            ..Self::default()
        }
    }
    pub fn append(&mut self, bytes: &[u8]) -> Result<(), NetworkHttpRefusal> {
        if self.closed {
            return Err(NetworkHttpRefusal::Closed);
        }
        if bytes.len() > REQUEST_BYTES - self.used {
            self.close();
            return Err(NetworkHttpRefusal::TooLarge);
        }
        self.bytes[self.used..self.used + bytes.len()].copy_from_slice(bytes);
        self.used += bytes.len();
        Ok(())
    }
    /// None means more bytes are required, never permission to submit a partial
    /// form. A terminal result erases the complete raw request immediately.
    pub fn take(
        &mut self,
        limits: NetworkSetupLimits,
    ) -> Result<Option<NetworkHttpRequest>, NetworkHttpRefusal> {
        if self.closed {
            return Err(NetworkHttpRefusal::Closed);
        }
        let result = parse(&self.bytes[..self.used], limits, self.readings);
        if !matches!(result, Ok(None)) {
            self.close();
        }
        result
    }
    fn close(&mut self) {
        self.bytes.zeroize();
        self.used = 0;
        self.closed = true;
    }
}
impl Drop for NetworkRequestBuffer {
    fn drop(&mut self) {
        self.close();
    }
}
fn parse(
    bytes: &[u8],
    limits: NetworkSetupLimits,
    readings: bool,
) -> Result<Option<NetworkHttpRequest>, NetworkHttpRefusal> {
    use NetworkHttpRefusal as E;
    let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") else {
        return if bytes.len() >= HEADER_BYTES {
            Err(E::TooLarge)
        } else {
            Ok(None)
        };
    };
    let body_start = end + 4;
    if body_start > HEADER_BYTES {
        return Err(E::TooLarge);
    }
    let headers = core::str::from_utf8(&bytes[..end]).map_err(|_| E::Malformed)?;
    let mut lines = headers.split("\r\n");
    let line = lines.next().ok_or(E::Malformed)?;
    let reading = match line {
        "GET / HTTP/1.1" | "GET / HTTP/1.0" if readings => Some(false),
        "GET /reading HTTP/1.1" | "GET /reading HTTP/1.0" if readings => Some(true),
        _ => None,
    };
    let history = if readings && line.starts_with("GET /history?after=") {
        let tail = line
            .strip_prefix("GET /history?after=")
            .ok_or(E::Malformed)?;
        let (number, version) = tail.split_once(' ').ok_or(E::Malformed)?;
        if !matches!(version, "HTTP/1.0" | "HTTP/1.1")
            || number.is_empty()
            || !number.bytes().all(|b| b.is_ascii_digit())
            || (number.len() > 1 && number.starts_with('0'))
        {
            return Err(E::Malformed);
        }
        Some(number.parse::<u32>().map_err(|_| E::TooLarge)?)
    } else {
        None
    };
    let network_snapshot = if readings {
        crate::network_custody::parse(line).map_err(|_| E::Malformed)?
    } else {
        None
    };
    let observed_offer = if readings && line.starts_with("GET /history/custody/offer?slot=") {
        let tail = line
            .strip_prefix("GET /history/custody/offer?slot=")
            .ok_or(E::Malformed)?;
        let (number, version) = tail.split_once(' ').ok_or(E::Malformed)?;
        if !matches!(version, "HTTP/1.0" | "HTTP/1.1")
            || number.len() != 1
            || !matches!(number.as_bytes()[0], b'0'..=b'5')
        {
            return Err(E::Malformed);
        }
        Some(number.as_bytes()[0] - b'0')
    } else {
        None
    };
    let observed_ack = if readings && line.starts_with("POST /history/custody/ack?slot=") {
        let tail = line
            .strip_prefix("POST /history/custody/ack?slot=")
            .ok_or(E::Malformed)?;
        let (number, version) = tail.split_once(' ').ok_or(E::Malformed)?;
        if !matches!(version, "HTTP/1.0" | "HTTP/1.1")
            || number.len() != 1
            || !matches!(number.as_bytes()[0], b'0'..=b'5')
        {
            return Err(E::Malformed);
        }
        Some(number.as_bytes()[0] - b'0')
    } else {
        None
    };
    let body_join_page =
        readings && matches!(line, "GET /body/join HTTP/1.0" | "GET /body/join HTTP/1.1");
    let body_join_post = readings
        && matches!(
            line,
            "POST /body/join HTTP/1.0" | "POST /body/join HTTP/1.1"
        );
    let body_join_options = readings
        && matches!(
            line,
            "OPTIONS /body/join HTTP/1.0" | "OPTIONS /body/join HTTP/1.1"
        );
    let body_join_ticket = if readings && line.starts_with("GET /body/join/result?id=") {
        let tail = line
            .strip_prefix("GET /body/join/result?id=")
            .ok_or(E::Malformed)?;
        let (number, version) = tail.split_once(' ').ok_or(E::Malformed)?;
        if !matches!(version, "HTTP/1.0" | "HTTP/1.1")
            || number.is_empty()
            || !number.bytes().all(|b| b.is_ascii_digit())
            || (number.len() > 1 && number.starts_with('0'))
        {
            return Err(E::Malformed);
        }
        Some(number.parse::<u32>().map_err(|_| E::TooLarge)?)
    } else {
        None
    };
    let page = match line {
        "GET /network HTTP/1.1" | "GET /network HTTP/1.0" => true,
        "POST /network HTTP/1.1" | "POST /network HTTP/1.0" => false,
        _ if body_join_post || observed_ack.is_some() => false,
        _ if reading.is_some()
            || observed_offer.is_some()
            || network_snapshot.is_some()
            || history.is_some()
            || body_join_page
            || body_join_options
            || body_join_ticket.is_some() =>
        {
            true
        }
        _ => return Err(E::UnsupportedRoute),
    };
    let (mut length, mut content_type) = (None, None);
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(E::Malformed)?;
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.bytes().any(|b| b.is_ascii_control() && b != b'\t')
        {
            return Err(E::Malformed);
        }
        let value = value.trim_matches([' ', '\t']);
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(E::Malformed);
        }
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(E::Malformed);
            }
            length = Some(value.parse::<usize>().map_err(|_| E::TooLarge)?);
        }
        if name.eq_ignore_ascii_case("content-type") && content_type.replace(value).is_some() {
            return Err(E::Malformed);
        }
    }
    let length = if page {
        length.unwrap_or(0)
    } else {
        length.ok_or(E::Malformed)?
    };
    if page && length != 0 {
        return Err(E::Malformed);
    }
    if length > REQUEST_BYTES - body_start {
        return Err(E::TooLarge);
    }
    if observed_ack.is_some() {
        if length == 0 {
            return Err(E::Malformed);
        }
        if length > crate::observed_custody::MAXIMUM_ACK_BYTES {
            return Err(E::TooLarge);
        }
    }
    let complete = body_start + length;
    if bytes.len() < complete {
        return Ok(None);
    }
    if bytes.len() != complete {
        return Err(E::Malformed);
    }
    if page {
        if let Some(request) = network_snapshot {
            return Ok(Some(NetworkHttpRequest::NetworkSnapshot(
                crate::network_custody::owned(request).map_err(|_| E::TooLarge)?,
            )));
        }
        if let Some(slot) = observed_offer {
            return Ok(Some(NetworkHttpRequest::ObservedOffer { slot }));
        }
        if body_join_page {
            return Ok(Some(NetworkHttpRequest::BodyJoinPage));
        }
        if body_join_options {
            return Ok(Some(NetworkHttpRequest::BodyJoinOptions));
        }
        if let Some(ticket) = body_join_ticket {
            return Ok(Some(NetworkHttpRequest::BodyJoinResult { ticket }));
        }
        if let Some(after) = history {
            return Ok(Some(NetworkHttpRequest::History { after }));
        }
        return Ok(Some(match reading {
            Some(json) => NetworkHttpRequest::Reading { json },
            None => NetworkHttpRequest::Page,
        }));
    }
    if body_join_post || observed_ack.is_some() {
        if !content_type.is_some_and(|value| value.eq_ignore_ascii_case("application/json")) {
            return Err(E::UnsupportedContentType);
        }
        let mut body = alloc::vec::Vec::new();
        body.try_reserve_exact(length).map_err(|_| E::TooLarge)?;
        body.extend_from_slice(&bytes[body_start..]);
        return Ok(Some(match observed_ack {
            Some(slot) => NetworkHttpRequest::ObservedAck { slot, bytes: body },
            None => NetworkHttpRequest::BodyJoin(body),
        }));
    }
    if !content_type.is_some_and(|v| v.eq_ignore_ascii_case("application/x-www-form-urlencoded")) {
        return Err(E::UnsupportedContentType);
    }
    NetworkSetupInput::from_form(&bytes[body_start..], limits)
        .map(|input| Some(NetworkHttpRequest::Join(input)))
        .map_err(E::Form)
}
