//! Bounded socket exchange. Root owns route installation and the connection's
//! absolute deadline; dropping this future cancels unflushed private submissions.
use crate::network_setup::*;
use embedded_io_async::{Read, Write};
use zeroize::Zeroizing;

/// Trusted Root's synchronous bounded Mask entry; a callback is not authority.
pub type ReadingProvider = fn(u64, bool) -> Option<heapless::String<2048>>;

pub struct NetworkSetupRoute<'a> {
    mailbox: &'a mut NetworkSetupMailbox,
    limits: NetworkSetupLimits,
    preparation_only: bool,
    reading_available: bool,
}
impl<'a> NetworkSetupRoute<'a> {
    /// Root installs this alongside its actual private credential source and
    /// admitted HTTP service. Input custody may precede SDK/Plan preparation;
    /// only a separately admitted control Play can cause a radio operation.
    /// Owning an HTTP connection grants no radio authority.
    pub fn new(
        mailbox: &'a mut NetworkSetupMailbox,
        limits: NetworkSetupLimits,
    ) -> Result<Self, NetworkSetupRefusal> {
        limits.validate()?;
        Ok(Self {
            mailbox,
            limits,
            preparation_only: false,
            reading_available: true,
        })
    }
    /// Root has a real control executor but no environmental Body/Face yet.
    /// Joining may start only after the response acknowledgement; reading routes
    /// remain unavailable rather than projecting a fictitious observation.
    pub fn bootstrap_control(
        mailbox: &'a mut NetworkSetupMailbox,
        limits: NetworkSetupLimits,
    ) -> Result<Self, NetworkSetupRefusal> {
        let mut route = Self::new(mailbox, limits)?;
        route.reading_available = false;
        Ok(route)
    }
    /// Root bootstrap input custody before a control Play is installed. Reading
    /// routes return unavailable rather than inventing a Body/Face basis.
    pub fn preparation_only(
        mailbox: &'a mut NetworkSetupMailbox,
        limits: NetworkSetupLimits,
    ) -> Result<Self, NetworkSetupRefusal> {
        let mut route = Self::new(mailbox, limits)?;
        route.preparation_only = true;
        route.reading_available = false;
        Ok(route)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum ExchangeRefusal {
    Transport,
    Closed,
    Framing(NetworkHttpRefusal),
    Submission(NetworkSetupRefusal),
}
#[derive(Debug, PartialEq, Eq)]
pub enum ExchangeOutcome {
    Reading,
    ReadingUnavailable,
    NetworkPage,
    SubmissionAcknowledged,
    NotFound,
}
/// No work/byte growth or retry: at most 1024 request bytes and 2048 response
/// bytes. Each successful I/O consumes at least one byte, except one final flush.
/// Root must bound stalled I/O with its independently admitted deadline.
pub async fn exchange<T: Read + Write>(
    io: &mut T,
    now: impl Fn() -> u64,
    route: Option<NetworkSetupRoute<'_>>,
) -> Result<ExchangeOutcome, ExchangeRefusal> {
    exchange_with_reading(io, now, route, |now, json| Some(crate::response(now, json))).await
}

/// Root supplies the selected, retained reading Mask rather than another sampler
/// or a second Face owner. The callback runs synchronously and returns owned,
/// bounded bytes before socket awaits. None means unavailable; no retry occurs.
/// Bootstrap routes still deny reads even when a callback exists. Supplying a
/// callback establishes neither Body membership nor permission to install it.
pub async fn exchange_with_reading<T: Read + Write>(
    io: &mut T,
    now: impl Fn() -> u64,
    route: Option<NetworkSetupRoute<'_>>,
    mut reading: impl FnMut(u64, bool) -> Option<heapless::String<2048>>,
) -> Result<ExchangeOutcome, ExchangeRefusal> {
    let mut response = heapless::String::new();
    exchange_buffered(io, now, route, &mut response, &mut reading).await
}

/// Response backing is prepared and retained by the selected service owner.
/// No request allocates; temporary rendering completes before socket awaits.
pub async fn exchange_buffered<T: Read + Write>(
    io: &mut T,
    now: impl Fn() -> u64,
    route: Option<NetworkSetupRoute<'_>>,
    response: &mut heapless::String<2048>,
    mut reading: impl FnMut(u64, bool) -> Option<heapless::String<2048>>,
) -> Result<ExchangeOutcome, ExchangeRefusal> {
    let limits = route
        .as_ref()
        .map(|route| route.limits)
        .unwrap_or(NetworkSetupLimits {
            minimum_credential_bytes: 8,
            maximum_credential_bytes: 64,
        });
    let mut request = NetworkRequestBuffer::for_http();
    let mut input = Zeroizing::new([0; 128]);
    let parsed = loop {
        let count = io
            .read(input.as_mut())
            .await
            .map_err(|_| ExchangeRefusal::Transport)?;
        if count == 0 {
            return Err(ExchangeRefusal::Closed);
        }
        request
            .append(&input[..count])
            .map_err(ExchangeRefusal::Framing)?;
        match request.take(limits) {
            Ok(Some(request)) => break Some(request),
            Ok(None) => {}
            Err(NetworkHttpRefusal::UnsupportedRoute) => break None,
            Err(error) => return Err(ExchangeRefusal::Framing(error)),
        }
    };
    match (parsed, route) {
        (Some(NetworkHttpRequest::Reading { .. }), Some(route)) if !route.reading_available => {
            write(io, b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
            Ok(ExchangeOutcome::ReadingUnavailable)
        }
        (Some(NetworkHttpRequest::Reading { json }), _) => {
            if render_into(&mut reading, now(), json, response) {
                write(io, response.as_bytes()).await?;
                Ok(ExchangeOutcome::Reading)
            } else {
                write(io, b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
                Ok(ExchangeOutcome::ReadingUnavailable)
            }
        }
        (Some(NetworkHttpRequest::Page), Some(route)) => {
            render_page_into(route.preparation_only, route.limits, response)?;
            write(io, response.as_bytes()).await?;
            Ok(ExchangeOutcome::NetworkPage)
        }
        (Some(NetworkHttpRequest::Join(input)), Some(route)) => {
            let pending = PendingNetworkResponse::prepare(route.mailbox, input)
                .map_err(ExchangeRefusal::Submission)?;
            render_ack_into(route.preparation_only, &pending, response)?;
            write(io, response.as_bytes()).await?;
            pending
                .response_flushed()
                .map_err(ExchangeRefusal::Submission)?;
            Ok(ExchangeOutcome::SubmissionAcknowledged)
        }
        _ => {
            write(
                io,
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await?;
            Ok(ExchangeOutcome::NotFound)
        }
    }
}
// Keep render scratch synchronous and outside the retained socket future. The
// page/acknowledgement cannot coexist with another response on this connection.
#[inline(never)]
fn render_page_into(
    preparation_only: bool,
    limits: NetworkSetupLimits,
    output: &mut heapless::String<2048>,
) -> Result<(), ExchangeRefusal> {
    output.clear();
    *output = if preparation_only {
        preparation_page(limits)
    } else {
        network_setup_page(limits)
    }
    .map_err(ExchangeRefusal::Submission)?;
    Ok(())
}
#[inline(never)]
fn render_ack_into(
    preparation_only: bool,
    pending: &PendingNetworkResponse<'_>,
    output: &mut heapless::String<2048>,
) -> Result<(), ExchangeRefusal> {
    output.clear();
    let acknowledgement = if preparation_only {
        pending.preparation_response()
    } else {
        pending.response()
    };
    output
        .push_str(acknowledgement.as_str())
        .map_err(|_| ExchangeRefusal::Submission(NetworkSetupRefusal::TooLarge))
}
#[inline(never)]
fn render_into(
    reading: &mut impl FnMut(u64, bool) -> Option<heapless::String<2048>>,
    now: u64,
    json: bool,
    output: &mut heapless::String<2048>,
) -> bool {
    output.clear();
    if let Some(response) = reading(now, json) {
        *output = response;
        true
    } else {
        false
    }
}

async fn write<T: Write>(io: &mut T, bytes: &[u8]) -> Result<(), ExchangeRefusal> {
    let mut written = 0;
    while written < bytes.len() {
        let count = io
            .write(&bytes[written..])
            .await
            .map_err(|_| ExchangeRefusal::Transport)?;
        if count == 0 {
            return Err(ExchangeRefusal::Closed);
        }
        written += count;
    }
    io.flush().await.map_err(|_| ExchangeRefusal::Transport)
}

#[cfg(test)]
mod tests;
