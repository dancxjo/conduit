//! Bounded socket exchange. Root owns route installation and the connection's
//! absolute deadline; dropping this future cancels unflushed private submissions.
use crate::network_setup::*;
use embedded_io_async::{Read, Write};
use zeroize::Zeroizing;

pub struct NetworkSetupRoute<'a> {
    mailbox: &'a mut NetworkSetupMailbox,
    limits: NetworkSetupLimits,
    preparation_only: bool,
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
        })
    }
    /// Root bootstrap input custody before a control Play is installed. Reading
    /// routes return unavailable rather than inventing a Body/Face basis.
    pub fn preparation_only(
        mailbox: &'a mut NetworkSetupMailbox,
        limits: NetworkSetupLimits,
    ) -> Result<Self, NetworkSetupRefusal> {
        let mut route = Self::new(mailbox, limits)?;
        route.preparation_only = true;
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
        (Some(NetworkHttpRequest::Reading { .. }), Some(route)) if route.preparation_only => {
            write(io, b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
            Ok(ExchangeOutcome::ReadingUnavailable)
        }
        (Some(NetworkHttpRequest::Reading { json }), _) => {
            write(io, crate::response(now(), json).as_bytes()).await?;
            Ok(ExchangeOutcome::Reading)
        }
        (Some(NetworkHttpRequest::Page), Some(route)) => {
            let page = if route.preparation_only {
                preparation_page(route.limits)
            } else {
                network_setup_page(route.limits)
            }
            .map_err(ExchangeRefusal::Submission)?;
            write(io, page.as_bytes()).await?;
            Ok(ExchangeOutcome::NetworkPage)
        }
        (Some(NetworkHttpRequest::Join(input)), Some(route)) => {
            let pending = PendingNetworkResponse::prepare(route.mailbox, input)
                .map_err(ExchangeRefusal::Submission)?;
            let response = if route.preparation_only {
                pending.preparation_response()
            } else {
                pending.response()
            };
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
