//! Exact bounded outbound HTTP client semantics.
//!
//! This module owns HTTP request/response behavior only. Resolution supplies
//! numeric endpoint, network, authority, TLS, proxy, grant, observation, and
//! limit facts before this code may commit an effect.

use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

use crate::HttpMethod;

pub const MAXIMUM_CLIENT_CONNECTIONS: u16 = 8;
pub const MAXIMUM_CLIENT_PENDING: u16 = 16;
pub const MAXIMUM_CLIENT_HEADERS: u16 = 64;
pub const MAXIMUM_CLIENT_HEADER_BYTES: usize = 16 * 1024;
pub const MAXIMUM_CLIENT_BODY_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_CLIENT_REDIRECTS: u16 = 8;
pub const MAXIMUM_CLIENT_EVIDENCE: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientScheme {
    Http,
    Https,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectPolicy {
    Return,
    FollowSameAuthority,
    FollowGrantedAuthorities,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitState {
    NotCommitted,
    Committed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientTerminal {
    Completed,
    DestinationDenied,
    StaleDnsObservation,
    StaleProviderObservation,
    CertificateFailed,
    HostnameFailed,
    RedirectLoop,
    RedirectLimit,
    DowngradeRejected,
    HeaderOverflow,
    BodyOverflow,
    TimedOut,
    PartialResponse,
    Cancelled,
    CommitUnknown,
    PoolExhausted,
    ProxyBindingRejected,
    ProviderLost,
    WorkOverflow,
    EvidenceOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientEvidenceKind {
    Admitted,
    RequestCommitted,
    ResponseHead,
    ResponseBodyChunk,
    RedirectObserved,
    RedirectAdmitted,
    CancellationObserved,
    CleanupCompleted,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientEvidence {
    pub kind: ClientEvidenceKind,
    pub status: u16,
    pub bytes: usize,
    pub redirect: u16,
    pub terminal: Option<ClientTerminal>,
    pub commit: CommitState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NumericEndpoint {
    pub address: [u8; 16],
    pub port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientLimits {
    pub maximum_connections: u16,
    pub maximum_pending: u16,
    pub maximum_request_headers: u16,
    pub maximum_request_header_bytes: usize,
    pub maximum_request_body_bytes: usize,
    pub maximum_response_headers: u16,
    pub maximum_response_header_bytes: usize,
    pub maximum_response_body_bytes: usize,
    pub maximum_body_chunk_bytes: usize,
    pub maximum_redirects: u16,
    pub maximum_retained_buffer_bytes: usize,
    pub maximum_timers: u16,
    pub maximum_work: usize,
    pub maximum_evidence_events: usize,
    pub deadline_ticks: u64,
    pub cleanup_ticks: u64,
}

impl ClientLimits {
    #[must_use]
    pub const fn checked_fixture() -> Self {
        Self {
            maximum_connections: 2,
            maximum_pending: 2,
            maximum_request_headers: 16,
            maximum_request_header_bytes: 4096,
            maximum_request_body_bytes: 16 * 1024,
            maximum_response_headers: 16,
            maximum_response_header_bytes: 4096,
            maximum_response_body_bytes: 64 * 1024,
            maximum_body_chunk_bytes: 4096,
            maximum_redirects: 4,
            maximum_retained_buffer_bytes: 80 * 1024,
            maximum_timers: 2,
            maximum_work: 128 * 1024,
            maximum_evidence_events: 32,
            deadline_ticks: 10_000,
            cleanup_ticks: 1_000,
        }
    }

    fn validate(self) -> bool {
        self.maximum_connections > 0
            && self.maximum_connections <= MAXIMUM_CLIENT_CONNECTIONS
            && self.maximum_pending > 0
            && self.maximum_pending <= MAXIMUM_CLIENT_PENDING
            && self.maximum_request_headers <= MAXIMUM_CLIENT_HEADERS
            && self.maximum_request_header_bytes > 0
            && self.maximum_request_header_bytes <= MAXIMUM_CLIENT_HEADER_BYTES
            && self.maximum_request_body_bytes <= MAXIMUM_CLIENT_BODY_BYTES
            && self.maximum_response_headers <= MAXIMUM_CLIENT_HEADERS
            && self.maximum_response_header_bytes > 0
            && self.maximum_response_header_bytes <= MAXIMUM_CLIENT_HEADER_BYTES
            && self.maximum_response_body_bytes <= MAXIMUM_CLIENT_BODY_BYTES
            && self.maximum_body_chunk_bytes > 0
            && self.maximum_body_chunk_bytes <= self.maximum_response_body_bytes
            && self.maximum_redirects <= MAXIMUM_CLIENT_REDIRECTS
            && self.maximum_retained_buffer_bytes
                >= self
                    .maximum_request_body_bytes
                    .saturating_add(self.maximum_response_body_bytes)
            && self.maximum_timers > 0
            && self.maximum_work > 0
            && self.maximum_evidence_events > 0
            && self.maximum_evidence_events <= MAXIMUM_CLIENT_EVIDENCE
            && self.deadline_ticks > 0
            && self.cleanup_ticks > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientBinding<'a> {
    pub endpoint: NumericEndpoint,
    pub authority: &'a str,
    pub network_resource: &'a str,
    pub outbound_grant: &'a str,
    pub tls_policy: &'a str,
    pub trust_handle: Option<&'a str>,
    pub client_certificate_handle: Option<&'a str>,
    pub client_private_key_handle: Option<&'a str>,
    pub proxy_resource: Option<&'a str>,
    pub dns_observation_fresh: bool,
    pub provider_observation_fresh: bool,
    pub destination_allowed: bool,
    pub limits: ClientLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientHeader<'a> {
    pub name: &'a str,
    pub value: &'a str,
    pub restricted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientRequest<'a> {
    pub method: HttpMethod,
    pub scheme: ClientScheme,
    pub authority: &'a str,
    pub target: &'a str,
    pub headers: &'a [ClientHeader<'a>],
    pub body: &'a [u8],
    pub redirects: RedirectPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixtureResponse<'a> {
    pub status: u16,
    pub header_count: u16,
    pub header_bytes: usize,
    pub body: &'a [u8],
    pub redirect_scheme: Option<ClientScheme>,
    pub redirect_authority: Option<&'a str>,
    pub redirect_target: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientFault {
    None,
    CertificateFailure,
    HostnameFailure,
    Timeout,
    PartialResponse(usize),
    CancelBeforeSend,
    CancelAfterSend,
    UnknownCommit,
    PoolExhaustion,
    ProxySurprise,
    ProviderLoss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeterministicExchange<'a> {
    pub responses: &'a [FixtureResponse<'a>],
    pub fault: ClientFault,
    pub observed_proxy: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientResult {
    pub terminal: ClientTerminal,
    pub status: Option<u16>,
    pub response_bytes: usize,
    pub redirects: u16,
    pub commit: CommitState,
    pub cleanup_completed: bool,
}

/// Opaque host-side trust bundle handle. Its path is never part of semantic
/// request values or normalized evidence.
pub struct ClientTrustHandle(PathBuf);

impl ClientTrustHandle {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

fn terminal_result(
    terminal: ClientTerminal,
    status: Option<u16>,
    response_bytes: usize,
    redirects: u16,
    commit: CommitState,
) -> ClientResult {
    ClientResult {
        terminal,
        status,
        response_bytes,
        redirects,
        commit,
        cleanup_completed: true,
    }
}

fn push_evidence(
    evidence: &mut [Option<ClientEvidence>],
    used: &mut usize,
    limit: usize,
    event: ClientEvidence,
) -> Result<(), ClientTerminal> {
    if *used >= limit || *used >= evidence.len() {
        return Err(ClientTerminal::EvidenceOverflow);
    }
    evidence[*used] = Some(event);
    *used += 1;
    Ok(())
}

fn header_bytes(headers: &[ClientHeader<'_>]) -> Option<usize> {
    headers.iter().try_fold(0_usize, |total, header| {
        total
            .checked_add(header.name.len())?
            .checked_add(header.value.len())?
            .checked_add(4)
    })
}

/// Execute one deterministic bounded request using caller-owned response and
/// evidence storage.
pub fn run_deterministic_client(
    binding: ClientBinding<'_>,
    request: ClientRequest<'_>,
    fixture: DeterministicExchange<'_>,
    response: &mut [u8],
    evidence: &mut [Option<ClientEvidence>],
) -> ClientResult {
    let mut used = 0;
    let mut redirects = 0_u16;
    let mut commit = CommitState::NotCommitted;
    let mut status = None;
    let mut response_bytes = 0;

    let reject = |terminal| terminal_result(terminal, None, 0, 0, CommitState::NotCommitted);
    if !binding.limits.validate()
        || binding.authority.is_empty()
        || binding.network_resource.is_empty()
        || binding.outbound_grant.is_empty()
        || binding.endpoint.port == 0
        || request.authority != binding.authority
        || !request.target.starts_with('/')
    {
        return reject(ClientTerminal::DestinationDenied);
    }
    if !binding.destination_allowed {
        return reject(ClientTerminal::DestinationDenied);
    }
    if !binding.dns_observation_fresh {
        return reject(ClientTerminal::StaleDnsObservation);
    }
    if !binding.provider_observation_fresh {
        return reject(ClientTerminal::StaleProviderObservation);
    }
    if request.scheme == ClientScheme::Https
        && (binding.tls_policy == "plaintext" || binding.trust_handle.is_none())
    {
        return reject(ClientTerminal::CertificateFailed);
    }
    if fixture.observed_proxy != binding.proxy_resource {
        return reject(ClientTerminal::ProxyBindingRejected);
    }
    if usize::from(request.headers.len() as u16)
        > usize::from(binding.limits.maximum_request_headers)
        || header_bytes(request.headers)
            .is_none_or(|bytes| bytes > binding.limits.maximum_request_header_bytes)
    {
        return reject(ClientTerminal::HeaderOverflow);
    }
    if request.body.len() > binding.limits.maximum_request_body_bytes {
        return reject(ClientTerminal::BodyOverflow);
    }
    if matches!(fixture.fault, ClientFault::PoolExhaustion) {
        return reject(ClientTerminal::PoolExhausted);
    }
    if matches!(fixture.fault, ClientFault::CancelBeforeSend) {
        return reject(ClientTerminal::Cancelled);
    }

    let base = ClientEvidence {
        kind: ClientEvidenceKind::Admitted,
        status: 0,
        bytes: 0,
        redirect: 0,
        terminal: None,
        commit,
    };
    if push_evidence(
        evidence,
        &mut used,
        binding.limits.maximum_evidence_events,
        base,
    )
    .is_err()
    {
        return reject(ClientTerminal::EvidenceOverflow);
    }
    commit = CommitState::Committed;
    if push_evidence(
        evidence,
        &mut used,
        binding.limits.maximum_evidence_events,
        ClientEvidence {
            kind: ClientEvidenceKind::RequestCommitted,
            bytes: request.body.len(),
            commit,
            ..base
        },
    )
    .is_err()
    {
        return terminal_result(ClientTerminal::EvidenceOverflow, None, 0, 0, commit);
    }

    let immediate_terminal = match fixture.fault {
        ClientFault::CertificateFailure => Some(ClientTerminal::CertificateFailed),
        ClientFault::HostnameFailure => Some(ClientTerminal::HostnameFailed),
        ClientFault::Timeout => Some(ClientTerminal::TimedOut),
        ClientFault::CancelAfterSend => Some(ClientTerminal::Cancelled),
        ClientFault::UnknownCommit => {
            commit = CommitState::Unknown;
            Some(ClientTerminal::CommitUnknown)
        }
        ClientFault::ProviderLoss => Some(ClientTerminal::ProviderLost),
        _ => None,
    };
    if let Some(terminal) = immediate_terminal {
        return terminal_result(terminal, None, 0, 0, commit);
    }

    let mut visited = [None; MAXIMUM_CLIENT_REDIRECTS as usize + 1];
    for fixture_response in fixture.responses {
        if fixture_response.header_count > binding.limits.maximum_response_headers
            || fixture_response.header_bytes > binding.limits.maximum_response_header_bytes
        {
            return terminal_result(
                ClientTerminal::HeaderOverflow,
                Some(fixture_response.status),
                response_bytes,
                redirects,
                commit,
            );
        }
        status = Some(fixture_response.status);
        if push_evidence(
            evidence,
            &mut used,
            binding.limits.maximum_evidence_events,
            ClientEvidence {
                kind: ClientEvidenceKind::ResponseHead,
                status: fixture_response.status,
                redirect: redirects,
                commit,
                ..base
            },
        )
        .is_err()
        {
            return terminal_result(
                ClientTerminal::EvidenceOverflow,
                status,
                response_bytes,
                redirects,
                commit,
            );
        }

        if (300..400).contains(&fixture_response.status)
            && fixture_response.redirect_target.is_some()
        {
            if request.redirects == RedirectPolicy::Return {
                break;
            }
            if redirects >= binding.limits.maximum_redirects {
                return terminal_result(
                    ClientTerminal::RedirectLimit,
                    status,
                    response_bytes,
                    redirects,
                    commit,
                );
            }
            let next_scheme = fixture_response.redirect_scheme.unwrap_or(request.scheme);
            let next_authority = fixture_response
                .redirect_authority
                .unwrap_or(request.authority);
            if request.scheme == ClientScheme::Https && next_scheme == ClientScheme::Http {
                return terminal_result(
                    ClientTerminal::DowngradeRejected,
                    status,
                    response_bytes,
                    redirects,
                    commit,
                );
            }
            if request.redirects == RedirectPolicy::FollowSameAuthority
                && next_authority != request.authority
            {
                return terminal_result(
                    ClientTerminal::DestinationDenied,
                    status,
                    response_bytes,
                    redirects,
                    commit,
                );
            }
            let target = fixture_response.redirect_target.unwrap_or("/");
            if visited[..usize::from(redirects)]
                .iter()
                .flatten()
                .any(|previous| *previous == target)
            {
                return terminal_result(
                    ClientTerminal::RedirectLoop,
                    status,
                    response_bytes,
                    redirects,
                    commit,
                );
            }
            visited[usize::from(redirects)] = Some(target);
            redirects += 1;
            continue;
        }

        let available = response.len().saturating_sub(response_bytes);
        let permitted = binding
            .limits
            .maximum_response_body_bytes
            .saturating_sub(response_bytes);
        if fixture_response.body.len() > available || fixture_response.body.len() > permitted {
            return terminal_result(
                ClientTerminal::BodyOverflow,
                status,
                response_bytes,
                redirects,
                commit,
            );
        }
        let copy_bytes = match fixture.fault {
            ClientFault::PartialResponse(bytes) => bytes.min(fixture_response.body.len()),
            _ => fixture_response.body.len(),
        };
        response[response_bytes..response_bytes + copy_bytes]
            .copy_from_slice(&fixture_response.body[..copy_bytes]);
        response_bytes += copy_bytes;
        for chunk in
            fixture_response.body[..copy_bytes].chunks(binding.limits.maximum_body_chunk_bytes)
        {
            if push_evidence(
                evidence,
                &mut used,
                binding.limits.maximum_evidence_events,
                ClientEvidence {
                    kind: ClientEvidenceKind::ResponseBodyChunk,
                    status: fixture_response.status,
                    bytes: chunk.len(),
                    redirect: redirects,
                    commit,
                    ..base
                },
            )
            .is_err()
            {
                return terminal_result(
                    ClientTerminal::EvidenceOverflow,
                    status,
                    response_bytes,
                    redirects,
                    commit,
                );
            }
        }
        if matches!(fixture.fault, ClientFault::PartialResponse(_)) {
            return terminal_result(
                ClientTerminal::PartialResponse,
                status,
                response_bytes,
                redirects,
                commit,
            );
        }
        break;
    }

    let work = request
        .body
        .len()
        .saturating_add(response_bytes)
        .saturating_add(header_bytes(request.headers).unwrap_or(usize::MAX));
    if work > binding.limits.maximum_work {
        return terminal_result(
            ClientTerminal::WorkOverflow,
            status,
            response_bytes,
            redirects,
            commit,
        );
    }
    terminal_result(
        ClientTerminal::Completed,
        status,
        response_bytes,
        redirects,
        commit,
    )
}

enum ClientIo {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Read for ClientIo {
    fn read(&mut self, destination: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(destination),
            Self::Tls(stream) => stream.read(destination),
        }
    }
}

impl Write for ClientIo {
    fn write(&mut self, source: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(source),
            Self::Tls(stream) => stream.write(source),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(stream) => stream.flush(),
            Self::Tls(stream) => stream.flush(),
        }
    }
}

fn endpoint_address(endpoint: NumericEndpoint) -> SocketAddr {
    let ip = if endpoint.address[..12] == [0; 12] {
        IpAddr::V4(Ipv4Addr::new(
            endpoint.address[12],
            endpoint.address[13],
            endpoint.address[14],
            endpoint.address[15],
        ))
    } else {
        IpAddr::V6(Ipv6Addr::from(endpoint.address))
    };
    SocketAddr::new(ip, endpoint.port)
}

fn method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Head => "HEAD",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Options => "OPTIONS",
    }
}

fn encode_request(
    request: ClientRequest<'_>,
    maximum_header_bytes: usize,
) -> Result<Vec<u8>, ClientTerminal> {
    if request.headers.iter().any(|header| {
        header.name.eq_ignore_ascii_case("host")
            || header.name.eq_ignore_ascii_case("content-length")
            || header.name.eq_ignore_ascii_case("connection")
            || header.name.contains(['\r', '\n'])
            || header.value.contains(['\r', '\n'])
    }) {
        return Err(ClientTerminal::HeaderOverflow);
    }
    let mut bytes = Vec::with_capacity(
        maximum_header_bytes
            .min(256_usize.saturating_add(request.body.len()))
            .saturating_add(request.body.len()),
    );
    write!(
        bytes,
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n",
        method_name(request.method),
        request.target,
        request.authority,
        request.body.len()
    )
    .map_err(|_| ClientTerminal::WorkOverflow)?;
    for header in request.headers {
        write!(bytes, "{}: {}\r\n", header.name, header.value)
            .map_err(|_| ClientTerminal::WorkOverflow)?;
    }
    bytes.extend_from_slice(b"\r\n");
    let header_len = bytes.len();
    if header_len > maximum_header_bytes {
        return Err(ClientTerminal::HeaderOverflow);
    }
    bytes.extend_from_slice(request.body);
    Ok(bytes)
}

fn load_client_config(handle: &ClientTrustHandle) -> Result<ClientConfig, ClientTerminal> {
    let file = File::open(&handle.0).map_err(|_| ClientTerminal::CertificateFailed)?;
    let certificates = rustls_pemfile::certs(&mut BufReader::new(file))
        .collect::<Result<Vec<CertificateDer<'static>>, _>>()
        .map_err(|_| ClientTerminal::CertificateFailed)?;
    let mut roots = RootCertStore::empty();
    for certificate in certificates {
        roots
            .add(certificate)
            .map_err(|_| ClientTerminal::CertificateFailed)?;
    }
    if roots.is_empty() {
        return Err(ClientTerminal::CertificateFailed);
    }
    Ok(ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth())
}

fn open_client_io(
    binding: ClientBinding<'_>,
    request: ClientRequest<'_>,
    trust: Option<&ClientTrustHandle>,
) -> Result<ClientIo, ClientTerminal> {
    let timeout = Duration::from_millis(binding.limits.deadline_ticks);
    let socket = TcpStream::connect_timeout(&endpoint_address(binding.endpoint), timeout)
        .map_err(|_| ClientTerminal::ProviderLost)?;
    socket
        .set_read_timeout(Some(timeout))
        .and_then(|()| socket.set_write_timeout(Some(timeout)))
        .map_err(|_| ClientTerminal::ProviderLost)?;
    if request.scheme == ClientScheme::Http {
        return Ok(ClientIo::Plain(socket));
    }
    let trust = trust.ok_or(ClientTerminal::CertificateFailed)?;
    let server_name = ServerName::try_from(request.authority.to_owned())
        .map_err(|_| ClientTerminal::HostnameFailed)?;
    let connection = ClientConnection::new(Arc::new(load_client_config(trust)?), server_name)
        .map_err(|_| ClientTerminal::CertificateFailed)?;
    Ok(ClientIo::Tls(Box::new(StreamOwned::new(
        connection, socket,
    ))))
}

fn parse_response(
    raw: &[u8],
    binding: ClientBinding<'_>,
    body_output: &mut [u8],
) -> Result<(u16, usize, usize), ClientTerminal> {
    let boundary = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(ClientTerminal::PartialResponse)?;
    let header_end = boundary + 4;
    if header_end > binding.limits.maximum_response_header_bytes {
        return Err(ClientTerminal::HeaderOverflow);
    }
    let head =
        std::str::from_utf8(&raw[..boundary]).map_err(|_| ClientTerminal::PartialResponse)?;
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_ascii_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or(ClientTerminal::PartialResponse)?;
    let mut header_count = 0_u16;
    let mut content_length = None;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or(ClientTerminal::PartialResponse)?;
        header_count = header_count
            .checked_add(1)
            .ok_or(ClientTerminal::HeaderOverflow)?;
        if name.eq_ignore_ascii_case("content-length") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| ClientTerminal::PartialResponse)?,
            );
        }
    }
    if header_count > binding.limits.maximum_response_headers {
        return Err(ClientTerminal::HeaderOverflow);
    }
    let body = &raw[header_end..];
    if content_length.is_some_and(|expected| expected != body.len()) {
        return Err(ClientTerminal::PartialResponse);
    }
    if body.len() > binding.limits.maximum_response_body_bytes || body.len() > body_output.len() {
        return Err(ClientTerminal::BodyOverflow);
    }
    body_output[..body.len()].copy_from_slice(body);
    Ok((status, header_count as usize, body.len()))
}

/// Execute one numeric-address Linux HTTP/1.1 request. This provider performs
/// no DNS, proxy discovery, cookie lookup, credential lookup, or retry.
pub fn run_linux_client(
    binding: ClientBinding<'_>,
    request: ClientRequest<'_>,
    trust: Option<&ClientTrustHandle>,
    body_output: &mut [u8],
    evidence: &mut [Option<ClientEvidence>],
) -> ClientResult {
    let fixture = DeterministicExchange {
        responses: &[],
        fault: ClientFault::None,
        observed_proxy: binding.proxy_resource,
    };
    let mut validation_body = [];
    let mut validation_evidence = [None; 2];
    let validation = run_deterministic_client(
        binding,
        request,
        fixture,
        &mut validation_body,
        &mut validation_evidence,
    );
    if validation.terminal != ClientTerminal::Completed {
        return validation;
    }

    let mut used = 0;
    let base = ClientEvidence {
        kind: ClientEvidenceKind::Admitted,
        status: 0,
        bytes: 0,
        redirect: 0,
        terminal: None,
        commit: CommitState::NotCommitted,
    };
    if push_evidence(
        evidence,
        &mut used,
        binding.limits.maximum_evidence_events,
        base,
    )
    .is_err()
    {
        return terminal_result(
            ClientTerminal::EvidenceOverflow,
            None,
            0,
            0,
            CommitState::NotCommitted,
        );
    }
    let request_bytes = match encode_request(request, binding.limits.maximum_request_header_bytes) {
        Ok(bytes) => bytes,
        Err(terminal) => {
            return terminal_result(terminal, None, 0, 0, CommitState::NotCommitted);
        }
    };
    let mut io = match open_client_io(binding, request, trust) {
        Ok(io) => io,
        Err(terminal) => {
            return terminal_result(terminal, None, 0, 0, CommitState::NotCommitted);
        }
    };
    if io
        .write_all(&request_bytes)
        .and_then(|()| io.flush())
        .is_err()
    {
        return terminal_result(
            ClientTerminal::CommitUnknown,
            None,
            0,
            0,
            CommitState::Unknown,
        );
    }
    if push_evidence(
        evidence,
        &mut used,
        binding.limits.maximum_evidence_events,
        ClientEvidence {
            kind: ClientEvidenceKind::RequestCommitted,
            bytes: request.body.len(),
            commit: CommitState::Committed,
            ..base
        },
    )
    .is_err()
    {
        return terminal_result(
            ClientTerminal::EvidenceOverflow,
            None,
            0,
            0,
            CommitState::Committed,
        );
    }

    let read_ceiling = binding
        .limits
        .maximum_response_header_bytes
        .saturating_add(binding.limits.maximum_response_body_bytes);
    let mut raw = Vec::with_capacity(read_ceiling.min(64 * 1024));
    match Read::by_ref(&mut io)
        .take(u64::try_from(read_ceiling.saturating_add(1)).unwrap_or(u64::MAX))
        .read_to_end(&mut raw)
    {
        Ok(_) if raw.len() <= read_ceiling => {}
        Ok(_) => {
            return terminal_result(
                ClientTerminal::BodyOverflow,
                None,
                0,
                0,
                CommitState::Committed,
            );
        }
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) =>
        {
            return terminal_result(ClientTerminal::TimedOut, None, 0, 0, CommitState::Committed);
        }
        Err(_) => {
            return terminal_result(
                if request.scheme == ClientScheme::Https {
                    ClientTerminal::CertificateFailed
                } else {
                    ClientTerminal::ProviderLost
                },
                None,
                0,
                0,
                CommitState::Committed,
            );
        }
    }
    let (status, _, body_bytes) = match parse_response(&raw, binding, body_output) {
        Ok(response) => response,
        Err(terminal) => {
            return terminal_result(terminal, None, 0, 0, CommitState::Committed);
        }
    };
    if push_evidence(
        evidence,
        &mut used,
        binding.limits.maximum_evidence_events,
        ClientEvidence {
            kind: ClientEvidenceKind::ResponseHead,
            status,
            commit: CommitState::Committed,
            ..base
        },
    )
    .is_err()
    {
        return terminal_result(
            ClientTerminal::EvidenceOverflow,
            Some(status),
            body_bytes,
            0,
            CommitState::Committed,
        );
    }
    for chunk in body_output[..body_bytes].chunks(binding.limits.maximum_body_chunk_bytes) {
        if push_evidence(
            evidence,
            &mut used,
            binding.limits.maximum_evidence_events,
            ClientEvidence {
                kind: ClientEvidenceKind::ResponseBodyChunk,
                status,
                bytes: chunk.len(),
                commit: CommitState::Committed,
                ..base
            },
        )
        .is_err()
        {
            return terminal_result(
                ClientTerminal::EvidenceOverflow,
                Some(status),
                body_bytes,
                0,
                CommitState::Committed,
            );
        }
    }
    terminal_result(
        ClientTerminal::Completed,
        Some(status),
        body_bytes,
        0,
        CommitState::Committed,
    )
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    fn binding() -> ClientBinding<'static> {
        ClientBinding {
            endpoint: NumericEndpoint {
                address: [0; 16],
                port: 8080,
            },
            authority: "service.test",
            network_resource: "network/fixture",
            outbound_grant: "grant/http-service",
            tls_policy: "fixture-trust",
            trust_handle: Some("secret/trust"),
            client_certificate_handle: None,
            client_private_key_handle: None,
            proxy_resource: None,
            dns_observation_fresh: true,
            provider_observation_fresh: true,
            destination_allowed: true,
            limits: ClientLimits::checked_fixture(),
        }
    }

    fn request(scheme: ClientScheme) -> ClientRequest<'static> {
        ClientRequest {
            method: HttpMethod::Get,
            scheme,
            authority: "service.test",
            target: "/health",
            headers: &[],
            body: b"",
            redirects: RedirectPolicy::Return,
        }
    }

    fn response(status: u16, body: &'static [u8]) -> FixtureResponse<'static> {
        FixtureResponse {
            status,
            header_count: 1,
            header_bytes: 32,
            body,
            redirect_scheme: None,
            redirect_authority: None,
            redirect_target: None,
        }
    }

    fn run(
        binding: ClientBinding<'_>,
        request: ClientRequest<'_>,
        responses: &[FixtureResponse<'_>],
        fault: ClientFault,
    ) -> ClientResult {
        let mut body = [0_u8; 128];
        let mut evidence = [None; 32];
        run_deterministic_client(
            binding,
            request,
            DeterministicExchange {
                responses,
                fault,
                observed_proxy: None,
            },
            &mut body,
            &mut evidence,
        )
    }

    #[test]
    fn success_preserves_http_and_https_without_ambient_inputs() {
        let response = response(200, b"ready");
        for scheme in [ClientScheme::Http, ClientScheme::Https] {
            let result = run(binding(), request(scheme), &[response], ClientFault::None);
            assert_eq!(result.terminal, ClientTerminal::Completed);
            assert_eq!(result.status, Some(200));
            assert_eq!(result.response_bytes, 5);
            assert_eq!(result.commit, CommitState::Committed);
        }
    }

    #[test]
    fn resolution_and_authority_fail_before_commit() {
        let mut denied = binding();
        denied.destination_allowed = false;
        assert_eq!(
            run(denied, request(ClientScheme::Http), &[], ClientFault::None).commit,
            CommitState::NotCommitted
        );
        let mut stale = binding();
        stale.provider_observation_fresh = false;
        assert_eq!(
            run(stale, request(ClientScheme::Http), &[], ClientFault::None).terminal,
            ClientTerminal::StaleProviderObservation
        );
        let surprise = DeterministicExchange {
            responses: &[],
            fault: ClientFault::None,
            observed_proxy: Some("proxy/ambient"),
        };
        let mut body = [0; 8];
        let mut evidence = [None; 8];
        assert_eq!(
            run_deterministic_client(
                binding(),
                request(ClientScheme::Http),
                surprise,
                &mut body,
                &mut evidence
            )
            .terminal,
            ClientTerminal::ProxyBindingRejected
        );
    }

    #[test]
    fn redirects_loop_limit_authority_and_downgrade_are_exact() {
        let redirected = FixtureResponse {
            status: 302,
            header_count: 1,
            header_bytes: 32,
            body: b"",
            redirect_scheme: Some(ClientScheme::Https),
            redirect_authority: Some("service.test"),
            redirect_target: Some("/next"),
        };
        let mut following = request(ClientScheme::Https);
        following.redirects = RedirectPolicy::FollowSameAuthority;
        let result = run(
            binding(),
            following,
            &[redirected, response(200, b"done")],
            ClientFault::None,
        );
        assert_eq!(result.terminal, ClientTerminal::Completed);
        assert_eq!(result.redirects, 1);

        let looped = run(
            binding(),
            following,
            &[redirected, redirected],
            ClientFault::None,
        );
        assert_eq!(looped.terminal, ClientTerminal::RedirectLoop);

        let downgrade = FixtureResponse {
            redirect_scheme: Some(ClientScheme::Http),
            ..redirected
        };
        assert_eq!(
            run(binding(), following, &[downgrade], ClientFault::None).terminal,
            ClientTerminal::DowngradeRejected
        );
    }

    #[test]
    fn partial_timeout_cancellation_unknown_commit_and_provider_loss_are_distinct() {
        let response = response(200, b"response");
        for (fault, terminal, commit) in [
            (
                ClientFault::PartialResponse(3),
                ClientTerminal::PartialResponse,
                CommitState::Committed,
            ),
            (
                ClientFault::Timeout,
                ClientTerminal::TimedOut,
                CommitState::Committed,
            ),
            (
                ClientFault::CancelBeforeSend,
                ClientTerminal::Cancelled,
                CommitState::NotCommitted,
            ),
            (
                ClientFault::CancelAfterSend,
                ClientTerminal::Cancelled,
                CommitState::Committed,
            ),
            (
                ClientFault::UnknownCommit,
                ClientTerminal::CommitUnknown,
                CommitState::Unknown,
            ),
            (
                ClientFault::ProviderLoss,
                ClientTerminal::ProviderLost,
                CommitState::Committed,
            ),
        ] {
            let result = run(binding(), request(ClientScheme::Http), &[response], fault);
            assert_eq!(result.terminal, terminal);
            assert_eq!(result.commit, commit);
            assert!(result.cleanup_completed);
        }
    }

    #[test]
    fn all_header_body_work_and_evidence_storage_is_bounded() {
        let mut small = binding();
        small.limits.maximum_response_body_bytes = 3;
        small.limits.maximum_body_chunk_bytes = 3;
        small.limits.maximum_retained_buffer_bytes =
            small.limits.maximum_request_body_bytes + small.limits.maximum_response_body_bytes;
        assert_eq!(
            run(
                small,
                request(ClientScheme::Http),
                &[response(200, b"large")],
                ClientFault::None
            )
            .terminal,
            ClientTerminal::BodyOverflow
        );

        let mut body = [0; 128];
        let mut evidence = [None; 1];
        assert_eq!(
            run_deterministic_client(
                binding(),
                request(ClientScheme::Http),
                DeterministicExchange {
                    responses: &[response(200, b"ok")],
                    fault: ClientFault::None,
                    observed_proxy: None
                },
                &mut body,
                &mut evidence
            )
            .terminal,
            ClientTerminal::EvidenceOverflow
        );
    }

    #[test]
    fn linux_client_uses_a_numeric_loopback_without_ambient_proxy_or_dns() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let read = stream.read(&mut request).unwrap();
            assert!(request[..read].starts_with(b"GET /health HTTP/1.1\r\nHost: service.test\r\n"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nready")
                .unwrap();
        });

        let mut exact = binding();
        exact.endpoint.address[12..].copy_from_slice(&[127, 0, 0, 1]);
        exact.endpoint.port = address.port();
        let mut body = [0_u8; 32];
        let mut evidence = [None; 16];
        let result = run_linux_client(
            exact,
            request(ClientScheme::Http),
            None,
            &mut body,
            &mut evidence,
        );
        server.join().unwrap();

        assert_eq!(result.terminal, ClientTerminal::Completed);
        assert_eq!(result.status, Some(200));
        assert_eq!(&body[..result.response_bytes], b"ready");
        assert!(evidence.iter().flatten().any(|event| {
            event.kind == ClientEvidenceKind::RequestCommitted
                && event.commit == CommitState::Committed
        }));
    }
}
