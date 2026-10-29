//! Bounded socket exchange. Root owns route installation and the connection's
//! absolute deadline; dropping this future cancels unflushed private submissions.
use crate::network_setup::*;
use embedded_io_async::{Read, Write};
use zeroize::Zeroizing;

/// Trusted Root's synchronous bounded Mask entry; a callback is not authority.
pub type ReadingProvider = fn(u64, bool, Option<u32>) -> Option<heapless::String<2048>>;

pub struct NetworkSetupRoute<'a> {
    mailbox: Option<&'a mut NetworkSetupMailbox>,
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
            mailbox: Some(mailbox),
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
/// Root-selected route policy, independent of mutable credential custody.
#[derive(Clone, Copy)]
pub enum NetworkSetupMode {
    Ready,
    BootstrapControl,
    PreparationOnly,
}
pub struct SelectedNetworkSetup<'a, 'channel> {
    writer: &'a NetworkSetupWriter<'channel>,
    limits: NetworkSetupLimits,
    mode: NetworkSetupMode,
}
impl<'a, 'channel> SelectedNetworkSetup<'a, 'channel> {
    pub fn new(
        writer: &'a NetworkSetupWriter<'channel>,
        limits: NetworkSetupLimits,
        mode: NetworkSetupMode,
    ) -> Result<Self, NetworkSetupRefusal> {
        limits.validate()?;
        Ok(Self {
            writer,
            limits,
            mode,
        })
    }
}
/// Retain this completion through the connection's bounded close. Only a parsed
/// credential submission acquires its guard; ordinary reads cannot block on it.
/// Dropping an unfinished exchange still cancels unflushed private input.
#[must_use = "retain credential custody until the connection's bounded close completes"]
pub struct CompletedExchange<'a> {
    outcome: ExchangeOutcome,
    _custody: Option<
        embassy_sync::mutex::MutexGuard<
            'a,
            embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
            NetworkSetupMailbox,
        >,
    >,
}
impl CompletedExchange<'_> {
    pub fn outcome(&self) -> &ExchangeOutcome {
        &self.outcome
    }
}
/// Parse one bounded request before acquiring mutation custody. Keep the exact
/// selected bootstrap/read policy even when no credential lock is needed.
/// Root must bound this entire future and retain its completion until close.
pub async fn exchange_selected_setup<'a, T: Read + Write>(
    io: &mut T,
    now: impl Fn() -> u64,
    selection: Option<SelectedNetworkSetup<'a, '_>>,
    response: &mut heapless::String<2048>,
    reading: impl FnMut(u64, bool) -> Option<heapless::String<2048>>,
    history: impl FnMut(u32) -> Option<heapless::String<2048>>,
) -> Result<CompletedExchange<'a>, ExchangeRefusal> {
    let limits = selection
        .as_ref()
        .map(|s| s.limits)
        .unwrap_or(NetworkSetupLimits {
            minimum_credential_bytes: 8,
            maximum_credential_bytes: 64,
        });
    let parsed = read_request(io, limits).await?;
    let mut custody = if matches!(&parsed, Some(NetworkHttpRequest::Join(_))) {
        match selection.as_ref() {
            Some(s) => Some(s.writer.lock().await),
            None => None,
        }
    } else {
        None
    };
    let outcome = {
        let route = selection.map(|s| NetworkSetupRoute {
            mailbox: custody.as_deref_mut(),
            limits: s.limits,
            preparation_only: matches!(s.mode, NetworkSetupMode::PreparationOnly),
            reading_available: matches!(s.mode, NetworkSetupMode::Ready),
        });
        respond_prepared(io, now, parsed, route, response, reading, history).await?
    };
    Ok(CompletedExchange {
        outcome,
        _custody: custody,
    })
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
    FaceExport,
    Reading,
    ReadingUnavailable,
    History,
    ObservedOffer,
    NetworkSnapshot,
    NetworkAck,
    ObservedAck,
    BodyJoin,
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
    exchange_buffered_with_history(io, now, route, response, &mut reading, |_| None).await
}
/// Independently installed read-only replay; bootstrap routes still deny it.
pub async fn exchange_buffered_with_history<T: Read + Write>(
    io: &mut T,
    now: impl Fn() -> u64,
    route: Option<NetworkSetupRoute<'_>>,
    response: &mut heapless::String<2048>,
    mut reading: impl FnMut(u64, bool) -> Option<heapless::String<2048>>,
    mut history: impl FnMut(u32) -> Option<heapless::String<2048>>,
) -> Result<ExchangeOutcome, ExchangeRefusal> {
    let limits = route
        .as_ref()
        .map(|route| route.limits)
        .unwrap_or(NetworkSetupLimits {
            minimum_credential_bytes: 8,
            maximum_credential_bytes: 64,
        });
    let parsed = read_request(io, limits).await?;
    respond_prepared(io, now, parsed, route, response, &mut reading, &mut history).await
}

async fn read_request<T: Read>(
    io: &mut T,
    limits: NetworkSetupLimits,
) -> Result<Option<NetworkHttpRequest>, ExchangeRefusal> {
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
    Ok(parsed)
}

async fn respond_prepared<T: Read + Write>(
    io: &mut T,
    now: impl Fn() -> u64,
    parsed: Option<NetworkHttpRequest>,
    route: Option<NetworkSetupRoute<'_>>,
    response: &mut heapless::String<2048>,
    mut reading: impl FnMut(u64, bool) -> Option<heapless::String<2048>>,
    mut history: impl FnMut(u32) -> Option<heapless::String<2048>>,
) -> Result<ExchangeOutcome, ExchangeRefusal> {
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
        (Some(NetworkHttpRequest::FaceExport(request)), route) => {
            let reply = if route.as_ref().is_none_or(|route| route.reading_available) {
                crate::face_export::dispatch(request)
            } else {
                crate::face_export::Reply::Unavailable
            };
            if !crate::face_export::render(reply, response) {
                return Err(ExchangeRefusal::Submission(NetworkSetupRefusal::TooLarge));
            }
            write(io, response.as_bytes()).await?;
            Ok(ExchangeOutcome::FaceExport)
        }
        (Some(NetworkHttpRequest::NetworkSnapshot(request)), route) => {
            let reply = if route.as_ref().is_none_or(|route| route.reading_available) {
                crate::network_custody::dispatch(request)
            } else {
                crate::network_custody::Reply::Unavailable
            };
            if !crate::network_custody::render(reply, response) {
                return Err(ExchangeRefusal::Submission(NetworkSetupRefusal::TooLarge));
            }
            write(io, response.as_bytes()).await?;
            Ok(ExchangeOutcome::NetworkSnapshot)
        }
        (Some(NetworkHttpRequest::NetworkAck(bytes)), route) => {
            let reply = if route.as_ref().is_none_or(|route| route.reading_available) {
                crate::network_custody::dispatch_ack(bytes)
            } else {
                crate::network_custody::Reply::Unavailable
            };
            if !crate::network_custody::render(reply, response) {
                return Err(ExchangeRefusal::Submission(NetworkSetupRefusal::TooLarge));
            }
            write(io, response.as_bytes()).await?;
            Ok(ExchangeOutcome::NetworkAck)
        }
        (Some(NetworkHttpRequest::ObservedAck { slot, bytes }), route) => {
            let reply = if route.as_ref().is_none_or(|route| route.reading_available) {
                crate::observed_custody::dispatch_ack(slot, bytes)
            } else {
                crate::observed_custody::Reply::Unavailable
            };
            if !crate::observed_custody::render(reply, response) {
                return Err(ExchangeRefusal::Submission(NetworkSetupRefusal::TooLarge));
            }
            write(io, response.as_bytes()).await?;
            Ok(ExchangeOutcome::ObservedAck)
        }
        (Some(NetworkHttpRequest::ObservedOffer { slot }), route) => {
            let reply = if route.as_ref().is_none_or(|route| route.reading_available) {
                crate::observed_custody::dispatch(slot)
            } else {
                crate::observed_custody::Reply::Unavailable
            };
            if !crate::observed_custody::render(reply, response) {
                return Err(ExchangeRefusal::Submission(NetworkSetupRefusal::TooLarge));
            }
            write(io, response.as_bytes()).await?;
            Ok(ExchangeOutcome::ObservedOffer)
        }
        (Some(NetworkHttpRequest::History { after }), route) => {
            response.clear();
            if route.as_ref().is_none_or(|r| r.reading_available)
                && render_history_into(&mut history, after, response)
            {
                write(io, response.as_bytes()).await?;
                return Ok(ExchangeOutcome::History);
            }
            write(io, b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
            Ok(ExchangeOutcome::ReadingUnavailable)
        }
        (
            Some(
                request @ (NetworkHttpRequest::BodyJoinPage
                | NetworkHttpRequest::BodyJoinOptions
                | NetworkHttpRequest::BodyJoin(_)
                | NetworkHttpRequest::BodyJoinResult { .. }),
            ),
            route,
        ) => {
            let available = route.as_ref().is_none_or(|route| route.reading_available)
                && crate::body_join::available();
            let rendered = if !available {
                crate::body_join::render(crate::body_join::Reply::Unavailable, response)
            } else {
                match request {
                    NetworkHttpRequest::BodyJoinPage => crate::body_join::page(response),
                    NetworkHttpRequest::BodyJoinOptions => crate::body_join::preflight(response),
                    NetworkHttpRequest::BodyJoin(bytes) => crate::body_join::render(
                        crate::body_join::dispatch(crate::body_join::Action::Submit(bytes)),
                        response,
                    ),
                    NetworkHttpRequest::BodyJoinResult { ticket } => crate::body_join::render(
                        crate::body_join::dispatch(crate::body_join::Action::Result(ticket)),
                        response,
                    ),
                    _ => false,
                }
            };
            if !rendered {
                return Err(ExchangeRefusal::Submission(NetworkSetupRefusal::TooLarge));
            }
            write(io, response.as_bytes()).await?;
            Ok(ExchangeOutcome::BodyJoin)
        }
        (Some(NetworkHttpRequest::Page), Some(route)) => {
            render_page_into(route.preparation_only, route.limits, response)?;
            write(io, response.as_bytes()).await?;
            Ok(ExchangeOutcome::NetworkPage)
        }
        (Some(NetworkHttpRequest::Join(input)), Some(route)) => {
            let pending = PendingNetworkResponse::prepare(
                route.mailbox.ok_or(ExchangeRefusal::Submission(
                    NetworkSetupRefusal::NoSubmission,
                ))?,
                input,
            )
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

#[inline(never)]
fn render_history_into(
    history: &mut impl FnMut(u32) -> Option<heapless::String<2048>>,
    after: u32,
    output: &mut heapless::String<2048>,
) -> bool {
    if let Some(value) = history(after) {
        *output = value;
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
