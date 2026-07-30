//! Bounded HTTP domain contracts and host-neutral serving interfaces.
//!
//! HTTP types, routing, listener state, TLS configuration, proxy trust, and
//! framework implementation details deliberately stay above `conduit-core`.
//! A resolved service pins every binding and finite limit before a listener
//! performs I/O.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Poll;
use std::time::{Duration, Instant};

use conduit_core::{
    Id, PinnedDescriptor, PlanArtifact, ReplacementSupport, SemanticHash, TransitionStateContract,
};
use conduit_runtime::{
    CompiledInHostService, HTTP_SERVE_ONCE_CONTRACT, Handler, HostedDrainObservation,
    HostedGenerationBinding, HostedTransitionGeneration, Registry, RegistryError, ResolutionError,
    ResolvedPlacementBinding, RunIo, RuntimeError, Value,
};
use rustls::pki_types::CertificateDer;
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use sha2::{Digest, Sha256};

pub const HTTP_PROFILE_VERSION: u16 = 1;
pub const HTTP_BACKEND_CONTRACT_ID: &str = "conduit/http-serving-backend";
pub const HTTP_LINUX_IMPLEMENTATION_ID: &str = "conduit/http.linux-rustls";
pub const HTTP_IN_MEMORY_IMPLEMENTATION_ID: &str = "conduit/http.in-memory";
pub const HTTP_CONSTRAINED_IMPLEMENTATION_ID: &str = "conduit/http.constrained";

/// Domain-owned HTTP value contracts. These are not core node kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpTypeKind {
    RequestHead,
    BodyChunk,
    ResponseHead,
    Header,
    Method,
    Uri,
    PathParameters,
    ExchangeIdentity,
    AuthenticatedPrincipal,
    TransportSecurity,
    ClientEvent,
    ServerViewUpdate,
    ProtocolFailure,
}

impl HttpTypeKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::RequestHead => "conduit.http/request-head",
            Self::BodyChunk => "conduit.http/body-chunk",
            Self::ResponseHead => "conduit.http/response-head",
            Self::Header => "conduit.http/header",
            Self::Method => "conduit.http/method",
            Self::Uri => "conduit.http/uri",
            Self::PathParameters => "conduit.http/path-parameters",
            Self::ExchangeIdentity => "conduit.http/exchange-identity",
            Self::AuthenticatedPrincipal => "conduit.http/authenticated-principal",
            Self::TransportSecurity => "conduit.http/transport-security",
            Self::ClientEvent => "conduit.http/client-event",
            Self::ServerViewUpdate => "conduit.http/server-view-update",
            Self::ProtocolFailure => "conduit.http/protocol-failure",
        }
    }
}

pub const HTTP_TYPE_CONTRACTS: [HttpTypeKind; 13] = [
    HttpTypeKind::RequestHead,
    HttpTypeKind::BodyChunk,
    HttpTypeKind::ResponseHead,
    HttpTypeKind::Header,
    HttpTypeKind::Method,
    HttpTypeKind::Uri,
    HttpTypeKind::PathParameters,
    HttpTypeKind::ExchangeIdentity,
    HttpTypeKind::AuthenticatedPrincipal,
    HttpTypeKind::TransportSecurity,
    HttpTypeKind::ClientEvent,
    HttpTypeKind::ServerViewUpdate,
    HttpTypeKind::ProtocolFailure,
];

/// Ordinary node/composite vocabulary supplied by the HTTP profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpNodeKind {
    Listener,
    Route,
    ResponseMux,
    AssetService,
    SessionGateway,
    ViewProjector,
    ServeComposite,
}

impl HttpNodeKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Listener => "conduit.http/http-listener",
            Self::Route => "conduit.http/http-route",
            Self::ResponseMux => "conduit.http/http-response-mux",
            Self::AssetService => "conduit.http/http-asset-service",
            Self::SessionGateway => "conduit.http/http-session-gateway",
            Self::ViewProjector => "conduit.http/view-projector",
            Self::ServeComposite => "conduit.http/serve",
        }
    }
}

pub const HTTP_NODE_CONTRACTS: [HttpNodeKind; 7] = [
    HttpNodeKind::Listener,
    HttpNodeKind::Route,
    HttpNodeKind::ResponseMux,
    HttpNodeKind::AssetService,
    HttpNodeKind::SessionGateway,
    HttpNodeKind::ViewProjector,
    HttpNodeKind::ServeComposite,
];

/// Exported boundary of the reusable `serve` composite. Its handler and view
/// children remain ordinary nodes and can be patched independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServeCompositeBoundary {
    pub request_output: Id<'static>,
    pub response_input: Id<'static>,
    pub client_event_output: Id<'static>,
    pub view_update_input: Id<'static>,
    pub evidence_output: Id<'static>,
}

pub const SERVE_COMPOSITE_BOUNDARY: ServeCompositeBoundary = ServeCompositeBoundary {
    request_output: Id("requests"),
    response_input: Id("responses"),
    client_event_output: Id("client-events"),
    view_update_input: Id("view-updates"),
    evidence_output: Id("evidence"),
};

/// Exact connection from an existing domain composite to a nested browser
/// view. It names exported ports; it does not copy the domain pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewProjectionBinding<'a> {
    pub domain_instance: Id<'a>,
    pub domain_state_port: Id<'a>,
    pub client_intent_port: Id<'a>,
    pub view_projector: Id<'a>,
    pub maximum_view_update_bytes: u32,
    pub maximum_pending_updates: u16,
}

pub fn validate_view_projection(binding: ViewProjectionBinding<'_>) -> Result<(), HttpReason> {
    for id in [
        binding.domain_instance,
        binding.domain_state_port,
        binding.client_intent_port,
        binding.view_projector,
    ] {
        Id::new(id.as_str()).map_err(|_| HttpReason::BindingMismatch)?;
    }
    if binding.maximum_view_update_bytes == 0 || binding.maximum_pending_updates == 0 {
        return Err(HttpReason::InvalidLimits);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

impl HttpMethod {
    fn parse(value: &str) -> Result<Self, HttpReason> {
        match value {
            "GET" => Ok(Self::Get),
            "HEAD" => Ok(Self::Head),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" => Ok(Self::Delete),
            "OPTIONS" => Ok(Self::Options),
            _ => Err(HttpReason::MalformedRequest),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpProtocol {
    Http11,
    Http2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpSecurityMode {
    Plaintext,
    DirectTls,
    TrustedProxyTls,
}

impl HttpSecurityMode {
    const fn tag(self) -> u8 {
        match self {
            Self::Plaintext => 1,
            Self::DirectTls => 2,
            Self::TrustedProxyTls => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpServingCapabilities {
    pub profile_version: u16,
    pub plaintext: bool,
    pub direct_tls: bool,
    pub trusted_proxy_tls: bool,
    pub http11: bool,
    pub http2: bool,
    pub websocket: bool,
    pub sse: bool,
    pub maximum_request_head_bytes: u32,
    pub maximum_request_body_bytes: u64,
    pub maximum_response_bytes: u64,
    pub maximum_connections: u16,
    pub maximum_sessions: u16,
    pub adapter_buffer_bytes: u64,
    pub backend_buffer_bytes: u64,
    pub kernel_buffer_bytes: u64,
    /// False means some backend or kernel capacity is observed rather than a
    /// hard enforcement ceiling and cannot satisfy a high-assurance request.
    pub complete_stack_hard_bounded: bool,
}

impl HttpServingCapabilities {
    #[must_use]
    pub fn accounted_memory_bytes(self) -> Option<u64> {
        self.adapter_buffer_bytes
            .checked_add(self.backend_buffer_bytes)?
            .checked_add(self.kernel_buffer_bytes)
    }

    #[must_use]
    pub const fn supports_security(self, mode: HttpSecurityMode) -> bool {
        match mode {
            HttpSecurityMode::Plaintext => self.plaintext,
            HttpSecurityMode::DirectTls => self.direct_tls,
            HttpSecurityMode::TrustedProxyTls => self.trusted_proxy_tls,
        }
    }
}

/// Every variable-sized or concurrent HTTP resource is an exact plan input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpServiceLimits {
    pub maximum_request_head_bytes: u32,
    pub maximum_request_body_bytes: u64,
    pub maximum_response_bytes: u64,
    pub maximum_header_count: u16,
    pub maximum_header_bytes: u32,
    pub maximum_connections: u16,
    pub maximum_queued_admissions: u16,
    pub maximum_live_handlers: u16,
    pub maximum_sessions: u16,
    pub maximum_session_queue_items: u16,
    pub maximum_session_queue_bytes: u64,
    pub maximum_evidence_events: u16,
    pub header_deadline_ticks: u64,
    pub body_deadline_ticks: u64,
    pub handler_deadline_ticks: u64,
    pub drain_deadline_ticks: u64,
    pub reserved_memory_bytes: u64,
}

impl HttpServiceLimits {
    fn validate(self) -> Result<(), HttpReason> {
        if self.maximum_request_head_bytes == 0
            || self.maximum_response_bytes == 0
            || self.maximum_header_count == 0
            || self.maximum_header_bytes == 0
            || self.maximum_connections == 0
            || self.maximum_queued_admissions == 0
            || self.maximum_live_handlers == 0
            || self.maximum_evidence_events == 0
            || self.header_deadline_ticks == 0
            || self.body_deadline_ticks == 0
            || self.handler_deadline_ticks == 0
            || self.drain_deadline_ticks == 0
            || self.reserved_memory_bytes == 0
            || self.maximum_live_handlers > self.maximum_connections
            || self.maximum_sessions > self.maximum_connections
            || (self.maximum_sessions > 0
                && (self.maximum_session_queue_items == 0 || self.maximum_session_queue_bytes == 0))
        {
            return Err(HttpReason::InvalidLimits);
        }
        Ok(())
    }
}

/// A trusted proxy is an exact transport peer, not a header convention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrustedProxy {
    pub identity: SemanticHash,
    pub address: IpAddr,
    pub accepts_forwarded_scheme: bool,
    pub accepts_forwarded_client: bool,
    pub accepts_forwarded_principal: bool,
}

/// Exact domain plan retained above the portable core plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedHttpService<'a> {
    pub identity: SemanticHash,
    pub service: PinnedDescriptor<'a>,
    pub backend: PinnedDescriptor<'a>,
    pub artifact: PlanArtifact<'a>,
    pub execution_profile: PinnedDescriptor<'a>,
    pub listen: &'a str,
    pub protocol: HttpProtocol,
    pub security: PinnedDescriptor<'a>,
    pub security_mode: HttpSecurityMode,
    pub certificate_identity: Option<SemanticHash>,
    pub trusted_proxy: Option<TrustedProxy>,
    pub grant: Id<'a>,
    pub secret_scope: Option<Id<'a>>,
    /// High-assurance profiles reject providers that only observe some
    /// library/kernel bounds.
    pub require_complete_stack_hard_bound: bool,
    pub limits: HttpServiceLimits,
}

impl ResolvedHttpService<'_> {
    #[must_use]
    pub fn computed_identity(&self) -> SemanticHash {
        let mut hash = Sha256::new();
        hash.update(b"conduit.resolved-http-service/v1\0");
        hash.update(self.service.id.as_str().as_bytes());
        hash.update(self.service.schema_version.to_be_bytes());
        hash.update(self.service.semantic_hash.as_bytes());
        hash.update(self.backend.id.as_str().as_bytes());
        hash.update(self.backend.schema_version.to_be_bytes());
        hash.update(self.backend.semantic_hash.as_bytes());
        hash.update(self.artifact.id.as_str().as_bytes());
        hash.update(self.artifact.digest.as_bytes());
        hash.update(self.execution_profile.id.as_str().as_bytes());
        hash.update(self.execution_profile.schema_version.to_be_bytes());
        hash.update(self.execution_profile.semantic_hash.as_bytes());
        hash.update(self.listen.as_bytes());
        hash.update([match self.protocol {
            HttpProtocol::Http11 => 1,
            HttpProtocol::Http2 => 2,
        }]);
        hash.update([self.security_mode.tag()]);
        hash.update(self.security.id.as_str().as_bytes());
        hash.update(self.security.schema_version.to_be_bytes());
        hash.update(self.security.semantic_hash.as_bytes());
        hash.update(
            self.certificate_identity
                .unwrap_or_else(|| SemanticHash::from_bytes([0; 32]))
                .as_bytes(),
        );
        hash.update(
            self.trusted_proxy
                .map_or_else(|| SemanticHash::from_bytes([0; 32]), |proxy| proxy.identity)
                .as_bytes(),
        );
        if let Some(proxy) = self.trusted_proxy {
            match proxy.address {
                IpAddr::V4(address) => hash.update(address.octets()),
                IpAddr::V6(address) => hash.update(address.octets()),
            }
            hash.update([
                u8::from(proxy.accepts_forwarded_scheme),
                u8::from(proxy.accepts_forwarded_client),
                u8::from(proxy.accepts_forwarded_principal),
            ]);
        }
        hash.update(self.grant.as_str().as_bytes());
        hash.update(self.secret_scope.map_or("", Id::as_str).as_bytes());
        hash.update([u8::from(self.require_complete_stack_hard_bound)]);
        encode_limits(&mut hash, self.limits);
        SemanticHash::from_bytes(hash.finalize().into())
    }

    pub fn validate(&self) -> Result<(), HttpReason> {
        self.limits.validate()?;
        if self.identity != self.computed_identity() {
            return Err(HttpReason::BindingMismatch);
        }
        if self.listen.is_empty() || Id::new(self.grant.as_str()).is_err() {
            return Err(HttpReason::BindingMismatch);
        }
        match self.security_mode {
            HttpSecurityMode::Plaintext => {
                if self.certificate_identity.is_some()
                    || self.trusted_proxy.is_some()
                    || self.secret_scope.is_some()
                {
                    return Err(HttpReason::SecurityBindingMismatch);
                }
            }
            HttpSecurityMode::DirectTls => {
                if self.certificate_identity.is_none()
                    || self.secret_scope.is_none()
                    || self.trusted_proxy.is_some()
                {
                    return Err(HttpReason::SecretHandleMissing);
                }
            }
            HttpSecurityMode::TrustedProxyTls => {
                if self.trusted_proxy.is_none()
                    || self.certificate_identity.is_some()
                    || self.secret_scope.is_some()
                {
                    return Err(HttpReason::ProxyTrustRejected);
                }
            }
        }
        Ok(())
    }
}

fn encode_limits(hash: &mut Sha256, limits: HttpServiceLimits) {
    hash.update(limits.maximum_request_head_bytes.to_be_bytes());
    hash.update(limits.maximum_request_body_bytes.to_be_bytes());
    hash.update(limits.maximum_response_bytes.to_be_bytes());
    hash.update(limits.maximum_header_count.to_be_bytes());
    hash.update(limits.maximum_header_bytes.to_be_bytes());
    hash.update(limits.maximum_connections.to_be_bytes());
    hash.update(limits.maximum_queued_admissions.to_be_bytes());
    hash.update(limits.maximum_live_handlers.to_be_bytes());
    hash.update(limits.maximum_sessions.to_be_bytes());
    hash.update(limits.maximum_session_queue_items.to_be_bytes());
    hash.update(limits.maximum_session_queue_bytes.to_be_bytes());
    hash.update(limits.maximum_evidence_events.to_be_bytes());
    hash.update(limits.header_deadline_ticks.to_be_bytes());
    hash.update(limits.body_deadline_ticks.to_be_bytes());
    hash.update(limits.handler_deadline_ticks.to_be_bytes());
    hash.update(limits.drain_deadline_ticks.to_be_bytes());
    hash.update(limits.reserved_memory_bytes.to_be_bytes());
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedHttpSelection<'a> {
    pub backend: PinnedDescriptor<'a>,
    pub artifact: PlanArtifact<'a>,
    pub execution_profile: PinnedDescriptor<'a>,
    pub endpoint: &'a str,
    pub security: PinnedDescriptor<'a>,
    pub security_mode: HttpSecurityMode,
    pub capabilities: HttpServingCapabilities,
}

pub fn validate_http_selection(
    service: &ResolvedHttpService<'_>,
    placement: &ResolvedPlacementBinding,
    selected: ResolvedHttpSelection<'_>,
) -> Result<(), HttpReason> {
    service.validate()?;
    if selected.capabilities.profile_version != HTTP_PROFILE_VERSION {
        return Err(HttpReason::UnsupportedProtocol);
    }
    if service.backend != selected.backend
        || placement.implementation_id != selected.backend.id.as_str()
        || placement.implementation_identity != selected.backend.semantic_hash
    {
        return Err(HttpReason::ImplementationMismatch);
    }
    if service.artifact != selected.artifact
        || !placement.artifacts.iter().any(|(id, digest)| {
            id == selected.artifact.id.as_str() && *digest == selected.artifact.digest
        })
    {
        return Err(HttpReason::ArtifactMismatch);
    }
    if service.execution_profile != selected.execution_profile {
        return Err(HttpReason::ProfileMismatch);
    }
    if !placement
        .authority_grants
        .iter()
        .any(|grant| grant == service.grant.as_str())
    {
        return Err(HttpReason::AuthorityDenied);
    }
    if service.listen != selected.endpoint
        || service.security != selected.security
        || service.security_mode != selected.security_mode
    {
        return Err(HttpReason::SecurityBindingMismatch);
    }
    let capabilities = selected.capabilities;
    if !capabilities.supports_security(selected.security_mode)
        || (service.protocol == HttpProtocol::Http11 && !capabilities.http11)
        || (service.protocol == HttpProtocol::Http2 && !capabilities.http2)
    {
        return Err(HttpReason::UnsupportedSecurity);
    }
    let limits = service.limits;
    let accounted = capabilities
        .accounted_memory_bytes()
        .ok_or(HttpReason::ResourceUnderaccounted)?;
    if capabilities.maximum_request_head_bytes < limits.maximum_request_head_bytes
        || capabilities.maximum_request_body_bytes < limits.maximum_request_body_bytes
        || capabilities.maximum_response_bytes < limits.maximum_response_bytes
        || capabilities.maximum_connections < limits.maximum_connections
        || capabilities.maximum_sessions < limits.maximum_sessions
        || accounted > limits.reserved_memory_bytes
        || (service.require_complete_stack_hard_bound && !capabilities.complete_stack_hard_bounded)
    {
        return Err(HttpReason::ResourceUnderaccounted);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpReason {
    UnsupportedProtocol,
    BindingMismatch,
    ImplementationMismatch,
    ArtifactMismatch,
    ProfileMismatch,
    SecurityBindingMismatch,
    UnsupportedSecurity,
    ResourceUnderaccounted,
    InvalidLimits,
    MalformedRequest,
    HeaderTooLarge,
    BodyTooLarge,
    ResponseTooLarge,
    AdmissionFull,
    SessionFull,
    QueueFull,
    Timeout,
    Cancelled,
    Closed,
    SecretHandleMissing,
    CertificateInvalid,
    ProxyTrustRejected,
    ForwardedHeaderRejected,
    CorrelationMismatch,
    UpgradeUnsupported,
    IoFailure,
    AuthorityDenied,
    EvidenceFull,
}

impl HttpReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedProtocol => "CND-HTTP-001",
            Self::BindingMismatch => "CND-HTTP-002",
            Self::ImplementationMismatch => "CND-HTTP-003",
            Self::ArtifactMismatch => "CND-HTTP-004",
            Self::ProfileMismatch => "CND-HTTP-005",
            Self::SecurityBindingMismatch => "CND-HTTP-006",
            Self::UnsupportedSecurity => "CND-HTTP-007",
            Self::ResourceUnderaccounted => "CND-HTTP-008",
            Self::InvalidLimits => "CND-HTTP-009",
            Self::MalformedRequest => "CND-HTTP-010",
            Self::HeaderTooLarge => "CND-HTTP-011",
            Self::BodyTooLarge => "CND-HTTP-012",
            Self::ResponseTooLarge => "CND-HTTP-013",
            Self::AdmissionFull => "CND-HTTP-014",
            Self::SessionFull => "CND-HTTP-015",
            Self::QueueFull => "CND-HTTP-016",
            Self::Timeout => "CND-HTTP-017",
            Self::Cancelled => "CND-HTTP-018",
            Self::Closed => "CND-HTTP-019",
            Self::SecretHandleMissing => "CND-HTTP-020",
            Self::CertificateInvalid => "CND-HTTP-021",
            Self::ProxyTrustRejected => "CND-HTTP-022",
            Self::ForwardedHeaderRejected => "CND-HTTP-023",
            Self::CorrelationMismatch => "CND-HTTP-024",
            Self::UpgradeUnsupported => "CND-HTTP-025",
            Self::IoFailure => "CND-HTTP-026",
            Self::AuthorityDenied => "CND-HTTP-027",
            Self::EvidenceFull => "CND-HTTP-028",
        }
    }
}

impl fmt::Display for HttpReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.code())
    }
}

impl std::error::Error for HttpReason {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpRequestHead {
    pub method: HttpMethod,
    pub target: String,
    pub headers: Vec<HttpHeader>,
}

impl HttpRequestHead {
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct HttpExchangeId {
    pub connection: u64,
    pub request: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpTransportSecurity {
    pub mode: HttpSecurityMode,
    pub encrypted: bool,
    pub authenticated_proxy: Option<SemanticHash>,
    pub client_address: IpAddr,
    pub authenticated_principal: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpRequest {
    pub exchange: HttpExchangeId,
    pub head: HttpRequestHead,
    pub body: Vec<u8>,
    pub security: HttpTransportSecurity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpResponsePart {
    pub exchange: HttpExchangeId,
    pub status: u16,
    pub headers: Vec<HttpHeader>,
    pub body: Vec<u8>,
    pub terminal: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpAsset<'a> {
    pub path: &'a str,
    pub media_type: &'a str,
    pub artifact_identity: SemanticHash,
    pub required_grant: Id<'a>,
    pub bytes: &'a [u8],
}

/// Resolve an immutable asset through the exact artifact/grant boundary.
pub fn resolve_asset<'a>(
    assets: &'a [HttpAsset<'a>],
    path: &str,
    artifact_identity: SemanticHash,
    grant: Id<'_>,
    maximum_response_bytes: u64,
) -> Result<Option<&'a HttpAsset<'a>>, HttpReason> {
    let Some(asset) = assets.iter().find(|asset| asset.path == path) else {
        return Ok(None);
    };
    if asset.artifact_identity != artifact_identity || asset.required_grant != grant {
        return Err(HttpReason::BindingMismatch);
    }
    if asset.bytes.len() as u64 > maximum_response_bytes {
        return Err(HttpReason::ResponseTooLarge);
    }
    Ok(Some(asset))
}

/// Certificate validity is a fresh host observation, not a fact inferred from
/// the presence of a secret handle.
pub fn validate_certificate_window(
    observed_tick: u64,
    not_before_tick: u64,
    not_after_tick: u64,
) -> Result<(), HttpReason> {
    if not_before_tick >= not_after_tick
        || observed_tick < not_before_tick
        || observed_tick >= not_after_tick
    {
        Err(HttpReason::CertificateInvalid)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionKind {
    WebSocket,
    ServerSentEvents,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpExchangeEvent {
    Request(HttpRequest),
    SessionOpened {
        exchange: HttpExchangeId,
        kind: SessionKind,
    },
    Cancelled {
        exchange: HttpExchangeId,
        reason: HttpReason,
    },
    Terminal {
        connection: u64,
        reason: Option<HttpReason>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpEvidenceKind {
    Bound,
    Accepted,
    RequestReceived,
    Routed,
    PressureEntered,
    PressureCleared,
    ResponseSent,
    SessionOpened,
    Cancelled,
    Rejected,
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedHttpEvidence {
    pub service_identity: SemanticHash,
    pub exchange: Option<HttpExchangeId>,
    pub kind: HttpEvidenceKind,
    pub reason: Option<HttpReason>,
    pub security_mode: HttpSecurityMode,
    pub encrypted: bool,
    pub proxy_authenticated: bool,
    /// A fresh exact grant was checked independently of TLS/proxy facts.
    pub conduit_authority_checked: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpServingAuthority<'a> {
    pub grant: Id<'a>,
    pub allowed: bool,
    pub current_tick: u64,
    pub valid_until_tick: u64,
}

impl HttpServingAuthority<'_> {
    fn validate(self, service: &ResolvedHttpService<'_>) -> Result<(), HttpReason> {
        if !self.allowed
            || self.grant != service.grant
            || self.current_tick >= self.valid_until_tick
        {
            Err(HttpReason::AuthorityDenied)
        } else {
            Ok(())
        }
    }
}

/// Executor-neutral host HTTP boundary.
pub trait HttpServingBackend {
    type Connection: Copy + Eq;
    type Error;
    type Evidence;

    fn capabilities(&self) -> HttpServingCapabilities;
    fn bind(
        &mut self,
        service: &ResolvedHttpService<'_>,
        authority: HttpServingAuthority<'_>,
    ) -> Result<(), Self::Error>;
    fn poll_accept(&mut self) -> Poll<Result<Self::Connection, Self::Error>>;
    fn poll_exchange(
        &mut self,
        connection: Self::Connection,
    ) -> Poll<Result<HttpExchangeEvent, Self::Error>>;
    fn poll_send(
        &mut self,
        connection: Self::Connection,
        response: &HttpResponsePart,
    ) -> Poll<Result<(), Self::Error>>;
    fn cancel(
        &mut self,
        connection: Self::Connection,
        exchange: HttpExchangeId,
    ) -> Result<(), Self::Error>;
    fn close(&mut self, connection: Self::Connection) -> Result<(), Self::Error>;
    fn take_evidence(&mut self) -> Option<Self::Evidence>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpRoute<'a> {
    pub id: Id<'a>,
    pub order: u16,
    pub method: HttpMethod,
    pub path_pattern: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteMatch {
    pub route: String,
    pub parameters: BTreeMap<String, String>,
}

/// Match the lowest explicit order, then canonical route ID. Input order is
/// never a hidden tie breaker.
pub fn match_route(
    routes: &[HttpRoute<'_>],
    method: HttpMethod,
    target: &str,
) -> Result<Option<RouteMatch>, HttpReason> {
    let path = target.split('?').next().unwrap_or(target);
    let mut matches = routes
        .iter()
        .filter(|route| route.method == method)
        .filter_map(|route| {
            match_path(route.path_pattern, path).map(|parameters| (route, parameters))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|(left, _), (right, _)| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });
    if matches.len() > 1
        && matches[0].0.order == matches[1].0.order
        && matches[0].0.id == matches[1].0.id
    {
        return Err(HttpReason::BindingMismatch);
    }
    Ok(matches.first().map(|(route, parameters)| RouteMatch {
        route: route.id.as_str().to_owned(),
        parameters: parameters.clone(),
    }))
}

fn match_path(pattern: &str, path: &str) -> Option<BTreeMap<String, String>> {
    let pattern = pattern.trim_matches('/').split('/').collect::<Vec<_>>();
    let path = path.trim_matches('/').split('/').collect::<Vec<_>>();
    if pattern.len() != path.len() {
        return None;
    }
    let mut parameters = BTreeMap::new();
    for (expected, actual) in pattern.into_iter().zip(path) {
        if expected.starts_with('{') && expected.ends_with('}') && expected.len() > 2 {
            parameters.insert(
                expected[1..expected.len() - 1].to_owned(),
                actual.to_owned(),
            );
        } else if expected != actual {
            return None;
        }
    }
    Some(parameters)
}

#[derive(Clone, Debug)]
struct InMemoryConnection {
    request: Option<HttpRequest>,
    response: Option<HttpResponsePart>,
    cancelled: bool,
}

#[derive(Clone, Debug)]
struct InMemorySession {
    kind: SessionKind,
    updates: VecDeque<Vec<u8>>,
    queued_bytes: u64,
}

/// Deterministic bounded backend used for routing, pressure, security, and
/// lifecycle conformance.
pub struct InMemoryHttpServingBackend {
    capabilities: HttpServingCapabilities,
    service: Option<OwnedServiceFacts>,
    next_connection: u64,
    queued: VecDeque<u64>,
    connections: BTreeMap<u64, InMemoryConnection>,
    sessions: BTreeMap<HttpExchangeId, InMemorySession>,
    evidence: VecDeque<HostedHttpEvidence>,
    pressure: bool,
    accepting: bool,
}

#[derive(Clone, Copy)]
struct OwnedServiceFacts {
    identity: SemanticHash,
    security_mode: HttpSecurityMode,
    trusted_proxy: Option<TrustedProxy>,
    limits: HttpServiceLimits,
}

impl InMemoryHttpServingBackend {
    #[must_use]
    pub fn new(capabilities: HttpServingCapabilities) -> Self {
        Self {
            capabilities,
            service: None,
            next_connection: 1,
            queued: VecDeque::new(),
            connections: BTreeMap::new(),
            sessions: BTreeMap::new(),
            evidence: VecDeque::new(),
            pressure: false,
            accepting: false,
        }
    }

    pub fn admit(
        &mut self,
        peer: IpAddr,
        raw_request: &[u8],
    ) -> Result<HttpExchangeId, HttpReason> {
        let facts = self.service.ok_or(HttpReason::Closed)?;
        if !self.accepting {
            return Err(HttpReason::Closed);
        }
        if self.connections.len() >= usize::from(facts.limits.maximum_connections)
            || self.queued.len() >= usize::from(facts.limits.maximum_queued_admissions)
        {
            self.ensure_evidence(2)?;
            self.record(
                None,
                HttpEvidenceKind::Rejected,
                Some(HttpReason::AdmissionFull),
            );
            self.set_pressure(true);
            return Err(HttpReason::AdmissionFull);
        }
        self.ensure_evidence(1)?;
        let connection = self.next_connection;
        self.next_connection = self.next_connection.saturating_add(1);
        let exchange = HttpExchangeId {
            connection,
            request: 1,
        };
        let request = parse_http_request(raw_request, exchange, peer, facts)?;
        self.connections.insert(
            connection,
            InMemoryConnection {
                request: Some(request),
                response: None,
                cancelled: false,
            },
        );
        self.queued.push_back(connection);
        self.record(Some(exchange), HttpEvidenceKind::Accepted, None);
        Ok(exchange)
    }

    pub fn open_session(
        &mut self,
        exchange: HttpExchangeId,
        kind: SessionKind,
    ) -> Result<(), HttpReason> {
        let facts = self.service.ok_or(HttpReason::Closed)?;
        self.ensure_evidence(1)?;
        if self.sessions.len() >= usize::from(facts.limits.maximum_sessions) {
            self.record(
                Some(exchange),
                HttpEvidenceKind::Rejected,
                Some(HttpReason::SessionFull),
            );
            return Err(HttpReason::SessionFull);
        }
        if kind == SessionKind::WebSocket && !self.capabilities.websocket
            || kind == SessionKind::ServerSentEvents && !self.capabilities.sse
        {
            return Err(HttpReason::UpgradeUnsupported);
        }
        self.sessions.insert(
            exchange,
            InMemorySession {
                kind,
                updates: VecDeque::new(),
                queued_bytes: 0,
            },
        );
        self.record(Some(exchange), HttpEvidenceKind::SessionOpened, None);
        Ok(())
    }

    pub fn send_session_update(
        &mut self,
        exchange: HttpExchangeId,
        update: &[u8],
    ) -> Result<(), HttpReason> {
        let facts = self.service.ok_or(HttpReason::Closed)?;
        self.ensure_evidence(1)?;
        let session = self.sessions.get_mut(&exchange).ok_or(HttpReason::Closed)?;
        let update_bytes = update.len() as u64;
        if session.updates.len() >= usize::from(facts.limits.maximum_session_queue_items)
            || session
                .queued_bytes
                .checked_add(update_bytes)
                .is_none_or(|bytes| bytes > facts.limits.maximum_session_queue_bytes)
        {
            self.record(
                Some(exchange),
                HttpEvidenceKind::PressureEntered,
                Some(HttpReason::QueueFull),
            );
            return Err(HttpReason::QueueFull);
        }
        session.queued_bytes += update_bytes;
        session.updates.push_back(update.to_vec());
        self.record(Some(exchange), HttpEvidenceKind::ResponseSent, None);
        Ok(())
    }

    pub fn take_session_update(
        &mut self,
        exchange: HttpExchangeId,
    ) -> Result<Option<Vec<u8>>, HttpReason> {
        let session = self.sessions.get_mut(&exchange).ok_or(HttpReason::Closed)?;
        let update = session.updates.pop_front();
        if let Some(update) = &update {
            session.queued_bytes = session.queued_bytes.saturating_sub(update.len() as u64);
        }
        Ok(update)
    }

    pub fn session_kind(&self, exchange: HttpExchangeId) -> Option<SessionKind> {
        self.sessions.get(&exchange).map(|session| session.kind)
    }

    pub fn expire(&mut self, connection: u64) -> Result<(), HttpReason> {
        self.ensure_evidence(if self.pressure { 2 } else { 1 })?;
        let exchange = self
            .connections
            .get(&connection)
            .and_then(|state| state.request.as_ref())
            .map(|request| request.exchange)
            .ok_or(HttpReason::Closed)?;
        self.connections.remove(&connection);
        self.sessions
            .retain(|exchange, _| exchange.connection != connection);
        self.queued.retain(|queued| *queued != connection);
        self.record(
            Some(exchange),
            HttpEvidenceKind::Cancelled,
            Some(HttpReason::Timeout),
        );
        self.set_pressure(false);
        Ok(())
    }

    /// Stop new admission and cancel every still-live exchange. The returned
    /// count is bounded by `maximum_connections`.
    pub fn shutdown(&mut self) -> Result<usize, HttpReason> {
        self.accepting = false;
        let connections = self.connections.keys().copied().collect::<Vec<_>>();
        self.ensure_evidence(connections.len())?;
        for connection in &connections {
            let exchange = self
                .connections
                .get(connection)
                .and_then(|state| state.request.as_ref())
                .map(|request| request.exchange);
            self.record(
                exchange,
                HttpEvidenceKind::Cancelled,
                Some(HttpReason::Cancelled),
            );
        }
        self.connections.clear();
        self.sessions.clear();
        self.queued.clear();
        self.service = None;
        self.pressure = false;
        Ok(connections.len())
    }

    /// Stop new admissions without destroying already accepted requests or
    /// sessions. Existing work remains available to the ordinary poll/close
    /// API until the transition's bounded drain deadline.
    pub fn begin_shutdown(&mut self) -> Result<usize, HttpReason> {
        self.service.ok_or(HttpReason::Closed)?;
        self.accepting = false;
        Ok(self.connections.len())
    }

    /// Cancel the exact bounded remainder after a failed/expired drain.
    pub fn finish_shutdown(&mut self) -> Result<usize, HttpReason> {
        self.shutdown()
    }

    /// Restore admissions after a pre-commit transition rollback.
    pub fn restore_admission(&mut self) -> Result<(), HttpReason> {
        self.service.ok_or(HttpReason::Closed)?;
        self.accepting = true;
        Ok(())
    }

    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    fn facts(&self) -> Result<OwnedServiceFacts, HttpReason> {
        self.service.ok_or(HttpReason::Closed)
    }

    fn ensure_evidence(&self, count: usize) -> Result<(), HttpReason> {
        let facts = self.service.ok_or(HttpReason::Closed)?;
        if self
            .evidence
            .len()
            .checked_add(count)
            .is_none_or(|required| required > usize::from(facts.limits.maximum_evidence_events))
        {
            Err(HttpReason::EvidenceFull)
        } else {
            Ok(())
        }
    }

    fn record(
        &mut self,
        exchange: Option<HttpExchangeId>,
        kind: HttpEvidenceKind,
        reason: Option<HttpReason>,
    ) {
        let Some(facts) = self.service else {
            return;
        };
        if self.evidence.len() < usize::from(facts.limits.maximum_evidence_events) {
            self.evidence.push_back(HostedHttpEvidence {
                service_identity: facts.identity,
                exchange,
                kind,
                reason,
                security_mode: facts.security_mode,
                encrypted: facts.security_mode != HttpSecurityMode::Plaintext,
                proxy_authenticated: facts.security_mode == HttpSecurityMode::TrustedProxyTls,
                conduit_authority_checked: true,
            });
        }
    }

    fn set_pressure(&mut self, pressure: bool) {
        if self.pressure != pressure {
            self.pressure = pressure;
            self.record(
                None,
                if pressure {
                    HttpEvidenceKind::PressureEntered
                } else {
                    HttpEvidenceKind::PressureCleared
                },
                None,
            );
        }
    }
}

struct InMemoryHttpGenerationState<'a> {
    backend: InMemoryHttpServingBackend,
    service: ResolvedHttpService<'a>,
    authority: HttpServingAuthority<'a>,
    prepared: bool,
    barrier_connections: Option<u32>,
}

/// Concrete HTTP request-generation participant for the generic transition
/// transaction. The cloneable handle lets the ordinary HTTP scheduler keep
/// serving old exchanges and send new admissions to the prepared generation;
/// lifecycle mutations remain owned by `HostedTransitionTransaction`.
pub struct InMemoryHttpTransitionGeneration<'a> {
    binding: HostedGenerationBinding<'a>,
    boundary: PinnedDescriptor<'a>,
    state: Rc<RefCell<InMemoryHttpGenerationState<'a>>>,
}

#[derive(Clone)]
pub struct InMemoryHttpTransitionHandle<'a> {
    state: Rc<RefCell<InMemoryHttpGenerationState<'a>>>,
}

impl<'a> InMemoryHttpTransitionGeneration<'a> {
    pub fn active(
        binding: HostedGenerationBinding<'a>,
        boundary: PinnedDescriptor<'a>,
        capabilities: HttpServingCapabilities,
        service: ResolvedHttpService<'a>,
        authority: HttpServingAuthority<'a>,
    ) -> Result<(Self, InMemoryHttpTransitionHandle<'a>), HttpReason> {
        validate_http_generation_binding(binding, boundary, service)?;
        let mut backend = InMemoryHttpServingBackend::new(capabilities);
        backend.bind(&service, authority)?;
        Ok(Self::from_parts(
            binding, boundary, backend, service, authority, true,
        ))
    }

    pub fn candidate(
        binding: HostedGenerationBinding<'a>,
        boundary: PinnedDescriptor<'a>,
        capabilities: HttpServingCapabilities,
        service: ResolvedHttpService<'a>,
        authority: HttpServingAuthority<'a>,
    ) -> Result<(Self, InMemoryHttpTransitionHandle<'a>), HttpReason> {
        validate_http_generation_binding(binding, boundary, service)?;
        Ok(Self::from_parts(
            binding,
            boundary,
            InMemoryHttpServingBackend::new(capabilities),
            service,
            authority,
            false,
        ))
    }

    fn from_parts(
        binding: HostedGenerationBinding<'a>,
        boundary: PinnedDescriptor<'a>,
        backend: InMemoryHttpServingBackend,
        service: ResolvedHttpService<'a>,
        authority: HttpServingAuthority<'a>,
        prepared: bool,
    ) -> (Self, InMemoryHttpTransitionHandle<'a>) {
        let state = Rc::new(RefCell::new(InMemoryHttpGenerationState {
            backend,
            service,
            authority,
            prepared,
            barrier_connections: None,
        }));
        (
            Self {
                binding,
                boundary,
                state: Rc::clone(&state),
            },
            InMemoryHttpTransitionHandle { state },
        )
    }
}

fn validate_http_generation_binding(
    binding: HostedGenerationBinding<'_>,
    boundary: PinnedDescriptor<'_>,
    service: ResolvedHttpService<'_>,
) -> Result<(), HttpReason> {
    service.validate()?;
    if binding.implementation != service.backend
        || binding.artifact != service.artifact.digest
        || !matches!(
            binding.replacement,
            ReplacementSupport::Quiescent {
                boundary: offered,
                maximum_ticks,
            } if offered == boundary
                && maximum_ticks >= service.limits.drain_deadline_ticks
        )
    {
        return Err(HttpReason::BindingMismatch);
    }
    Ok(())
}

impl InMemoryHttpTransitionHandle<'_> {
    pub fn admit(&self, peer: IpAddr, raw_request: &[u8]) -> Result<HttpExchangeId, HttpReason> {
        self.state.borrow_mut().backend.admit(peer, raw_request)
    }

    pub fn poll_accept(&self) -> Poll<Result<u64, HttpReason>> {
        self.state.borrow_mut().backend.poll_accept()
    }

    pub fn poll_exchange(&self, connection: u64) -> Poll<Result<HttpExchangeEvent, HttpReason>> {
        self.state.borrow_mut().backend.poll_exchange(connection)
    }

    pub fn poll_send(
        &self,
        connection: u64,
        response: &HttpResponsePart,
    ) -> Poll<Result<(), HttpReason>> {
        self.state
            .borrow_mut()
            .backend
            .poll_send(connection, response)
    }

    pub fn close(&self, connection: u64) -> Result<(), HttpReason> {
        self.state.borrow_mut().backend.close(connection)
    }

    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.state.borrow().backend.connection_count()
    }
}

impl HostedTransitionGeneration for InMemoryHttpTransitionGeneration<'_> {
    fn binding(&self) -> HostedGenerationBinding<'_> {
        self.binding
    }

    fn prepare(&mut self) -> Result<(), Id<'static>> {
        let mut state = self.state.borrow_mut();
        if state.prepared {
            return Ok(());
        }
        let service = state.service;
        let authority = state.authority;
        state
            .backend
            .bind(&service, authority)
            .map_err(|_| Id("http/prepare-failed"))?;
        state.prepared = true;
        Ok(())
    }

    fn stop_admission(&mut self, boundary: PinnedDescriptor<'_>) -> Result<(), Id<'static>> {
        if boundary != self.boundary {
            return Err(Id("http/boundary-mismatch"));
        }
        let mut state = self.state.borrow_mut();
        let connections = state
            .backend
            .begin_shutdown()
            .map_err(|_| Id("http/barrier-failed"))?;
        state.barrier_connections =
            Some(u32::try_from(connections).map_err(|_| Id("http/drain-overflow"))?);
        Ok(())
    }

    fn drain(
        &mut self,
        boundary: PinnedDescriptor<'_>,
    ) -> Result<HostedDrainObservation, Id<'static>> {
        if boundary != self.boundary {
            return Err(Id("http/boundary-mismatch"));
        }
        let state = self.state.borrow();
        let initial = state
            .barrier_connections
            .ok_or(Id("http/barrier-missing"))?;
        let remaining = u32::try_from(state.backend.connection_count())
            .map_err(|_| Id("http/drain-overflow"))?;
        let completed = initial
            .checked_sub(remaining)
            .ok_or(Id("http/drain-invalid"))?;
        Ok(HostedDrainObservation {
            remaining_values: remaining,
            remaining_operations: remaining,
            drained_values: completed,
            rejected_values: 0,
            lost_values: 0,
            completed_operations: completed,
            cancelled_operations: 0,
        })
    }

    fn export_state(
        &mut self,
        _: TransitionStateContract<'_>,
        _: &mut [u8],
    ) -> Result<usize, Id<'static>> {
        Err(Id("http/state-unsupported"))
    }

    fn import_state(
        &mut self,
        _: TransitionStateContract<'_>,
        _: &[u8],
    ) -> Result<usize, Id<'static>> {
        Err(Id("http/state-unsupported"))
    }

    fn accept_replayed_value(
        &mut self,
        _: u64,
        _: &[u8],
        _: Option<conduit_runtime::RuntimeValueEnvelope>,
        _: bool,
    ) -> Result<(), Id<'static>> {
        Err(Id("http/replay-unsupported"))
    }

    fn retire(&mut self) -> Result<(), Id<'static>> {
        let remaining = self
            .state
            .borrow_mut()
            .backend
            .finish_shutdown()
            .map_err(|_| Id("http/retire-failed"))?;
        if remaining == 0 {
            Ok(())
        } else {
            Err(Id("http/retire-undrained"))
        }
    }

    fn abort_candidate(&mut self) -> Result<(), Id<'static>> {
        let mut state = self.state.borrow_mut();
        if state.prepared {
            state
                .backend
                .shutdown()
                .map_err(|_| Id("http/abort-failed"))?;
            state.prepared = false;
        }
        Ok(())
    }

    fn restore_old(&mut self) -> Result<(), Id<'static>> {
        self.state
            .borrow_mut()
            .backend
            .restore_admission()
            .map_err(|_| Id("http/restore-failed"))
    }
}

impl HttpServingBackend for InMemoryHttpServingBackend {
    type Connection = u64;
    type Error = HttpReason;
    type Evidence = HostedHttpEvidence;

    fn capabilities(&self) -> HttpServingCapabilities {
        self.capabilities
    }

    fn bind(
        &mut self,
        service: &ResolvedHttpService<'_>,
        authority: HttpServingAuthority<'_>,
    ) -> Result<(), Self::Error> {
        service.validate()?;
        authority.validate(service)?;
        if service.backend.id.as_str() != HTTP_IN_MEMORY_IMPLEMENTATION_ID {
            return Err(HttpReason::ImplementationMismatch);
        }
        if !self.capabilities.supports_security(service.security_mode) {
            return Err(HttpReason::UnsupportedSecurity);
        }
        if service.require_complete_stack_hard_bound
            && !self.capabilities.complete_stack_hard_bounded
        {
            return Err(HttpReason::ResourceUnderaccounted);
        }
        self.service = Some(OwnedServiceFacts {
            identity: service.identity,
            security_mode: service.security_mode,
            trusted_proxy: service.trusted_proxy,
            limits: service.limits,
        });
        self.accepting = true;
        self.record(None, HttpEvidenceKind::Bound, None);
        Ok(())
    }

    fn poll_accept(&mut self) -> Poll<Result<Self::Connection, Self::Error>> {
        if self.pressure {
            if let Err(error) = self.ensure_evidence(1) {
                return Poll::Ready(Err(error));
            }
        }
        if let Some(connection) = self.queued.pop_front() {
            self.set_pressure(false);
            Poll::Ready(Ok(connection))
        } else {
            Poll::Pending
        }
    }

    fn poll_exchange(
        &mut self,
        connection: Self::Connection,
    ) -> Poll<Result<HttpExchangeEvent, Self::Error>> {
        let Some(state) = self.connections.get_mut(&connection) else {
            return Poll::Ready(Err(HttpReason::Closed));
        };
        if state.cancelled {
            return Poll::Ready(Err(HttpReason::Cancelled));
        }
        if let Err(error) = self.ensure_evidence(1) {
            return Poll::Ready(Err(error));
        }
        let Some(state) = self.connections.get_mut(&connection) else {
            return Poll::Ready(Err(HttpReason::Closed));
        };
        let Some(request) = state.request.take() else {
            return Poll::Pending;
        };
        self.record(
            Some(request.exchange),
            HttpEvidenceKind::RequestReceived,
            None,
        );
        Poll::Ready(Ok(HttpExchangeEvent::Request(request)))
    }

    fn poll_send(
        &mut self,
        connection: Self::Connection,
        response: &HttpResponsePart,
    ) -> Poll<Result<(), Self::Error>> {
        let limits = match self.facts() {
            Ok(facts) => facts.limits,
            Err(error) => return Poll::Ready(Err(error)),
        };
        if let Err(error) = validate_response(response, limits) {
            return Poll::Ready(Err(error));
        }
        if let Err(error) = self.ensure_evidence(1) {
            return Poll::Ready(Err(error));
        }
        let Some(state) = self.connections.get_mut(&connection) else {
            return Poll::Ready(Err(HttpReason::Closed));
        };
        if response.exchange.connection != connection {
            return Poll::Ready(Err(HttpReason::CorrelationMismatch));
        }
        state.response = Some(response.clone());
        self.record(
            Some(response.exchange),
            HttpEvidenceKind::ResponseSent,
            None,
        );
        Poll::Ready(Ok(()))
    }

    fn cancel(
        &mut self,
        connection: Self::Connection,
        exchange: HttpExchangeId,
    ) -> Result<(), Self::Error> {
        self.ensure_evidence(1)?;
        let state = self
            .connections
            .get_mut(&connection)
            .ok_or(HttpReason::Closed)?;
        if exchange.connection != connection {
            return Err(HttpReason::CorrelationMismatch);
        }
        state.cancelled = true;
        self.record(
            Some(exchange),
            HttpEvidenceKind::Cancelled,
            Some(HttpReason::Cancelled),
        );
        Ok(())
    }

    fn close(&mut self, connection: Self::Connection) -> Result<(), Self::Error> {
        self.ensure_evidence(if self.pressure { 2 } else { 1 })?;
        let state = self
            .connections
            .remove(&connection)
            .ok_or(HttpReason::Closed)?;
        self.sessions
            .retain(|exchange, _| exchange.connection != connection);
        let exchange = state
            .request
            .as_ref()
            .map(|request| request.exchange)
            .or_else(|| state.response.as_ref().map(|response| response.exchange));
        self.queued.retain(|queued| *queued != connection);
        self.record(exchange, HttpEvidenceKind::Closed, None);
        self.set_pressure(false);
        Ok(())
    }

    fn take_evidence(&mut self) -> Option<Self::Evidence> {
        self.evidence.pop_front()
    }
}

fn parse_http_request(
    raw: &[u8],
    exchange: HttpExchangeId,
    peer: IpAddr,
    facts: OwnedServiceFacts,
) -> Result<HttpRequest, HttpReason> {
    let boundary = find_header_boundary(raw).ok_or(HttpReason::MalformedRequest)?;
    if boundary as u32 > facts.limits.maximum_request_head_bytes {
        return Err(HttpReason::HeaderTooLarge);
    }
    let head = std::str::from_utf8(&raw[..boundary]).map_err(|_| HttpReason::MalformedRequest)?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or(HttpReason::MalformedRequest)?;
    let mut request_parts = request_line.split(' ');
    let method = HttpMethod::parse(request_parts.next().ok_or(HttpReason::MalformedRequest)?)?;
    let target = request_parts
        .next()
        .filter(|target| target.starts_with('/'))
        .ok_or(HttpReason::MalformedRequest)?
        .to_owned();
    if request_parts.next() != Some("HTTP/1.1") || request_parts.next().is_some() {
        return Err(HttpReason::MalformedRequest);
    }
    let mut headers = Vec::new();
    let mut header_bytes = 0usize;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once(':').ok_or(HttpReason::MalformedRequest)?;
        let name = name.trim();
        let value = value.trim();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value.bytes().any(|byte| byte < b' ' && byte != b'\t')
        {
            return Err(HttpReason::MalformedRequest);
        }
        header_bytes = header_bytes.saturating_add(name.len() + value.len());
        headers.push(HttpHeader {
            name: name.to_ascii_lowercase(),
            value: value.to_owned(),
        });
    }
    if headers.len() > usize::from(facts.limits.maximum_header_count)
        || header_bytes as u32 > facts.limits.maximum_header_bytes
    {
        return Err(HttpReason::HeaderTooLarge);
    }
    let content_length = headers
        .iter()
        .find(|header| header.name == "content-length")
        .map(|header| {
            header
                .value
                .parse::<usize>()
                .map_err(|_| HttpReason::MalformedRequest)
        })
        .transpose()?
        .unwrap_or(0);
    let body = &raw[boundary + 4..];
    if content_length != body.len() {
        return Err(HttpReason::MalformedRequest);
    }
    if body.len() as u64 > facts.limits.maximum_request_body_bytes {
        return Err(HttpReason::BodyTooLarge);
    }
    let mut security = HttpTransportSecurity {
        mode: facts.security_mode,
        encrypted: facts.security_mode != HttpSecurityMode::Plaintext,
        authenticated_proxy: None,
        client_address: peer,
        authenticated_principal: None,
    };
    apply_proxy_headers(&headers, peer, facts, &mut security)?;
    Ok(HttpRequest {
        exchange,
        head: HttpRequestHead {
            method,
            target,
            headers,
        },
        body: body.to_vec(),
        security,
    })
}

fn find_header_boundary(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn apply_proxy_headers(
    headers: &[HttpHeader],
    peer: IpAddr,
    facts: OwnedServiceFacts,
    security: &mut HttpTransportSecurity,
) -> Result<(), HttpReason> {
    let forwarded = headers.iter().any(|header| {
        matches!(
            header.name.as_str(),
            "forwarded" | "x-forwarded-proto" | "x-forwarded-for" | "x-authenticated-principal"
        )
    });
    if facts.security_mode != HttpSecurityMode::TrustedProxyTls {
        if forwarded {
            return Err(HttpReason::ForwardedHeaderRejected);
        }
        return Ok(());
    }
    let proxy = facts.trusted_proxy.ok_or(HttpReason::ProxyTrustRejected)?;
    if proxy.address != peer {
        return Err(HttpReason::ProxyTrustRejected);
    }
    security.authenticated_proxy = Some(proxy.identity);
    if let Some(scheme) = headers
        .iter()
        .find(|header| header.name == "x-forwarded-proto")
        .map(|header| header.value.as_str())
    {
        if !proxy.accepts_forwarded_scheme || scheme != "https" {
            return Err(HttpReason::ForwardedHeaderRejected);
        }
    } else {
        return Err(HttpReason::ForwardedHeaderRejected);
    }
    if let Some(client) = headers
        .iter()
        .find(|header| header.name == "x-forwarded-for")
        .map(|header| header.value.as_str())
    {
        if !proxy.accepts_forwarded_client {
            return Err(HttpReason::ForwardedHeaderRejected);
        }
        security.client_address = client
            .parse()
            .map_err(|_| HttpReason::ForwardedHeaderRejected)?;
    }
    if let Some(principal) = headers
        .iter()
        .find(|header| header.name == "x-authenticated-principal")
        .map(|header| header.value.as_str())
    {
        if !proxy.accepts_forwarded_principal {
            return Err(HttpReason::ForwardedHeaderRejected);
        }
        security.authenticated_principal = Some(principal.to_owned());
    }
    Ok(())
}

/// Opaque host-side certificate or private-key file handle.
pub struct SecretFileHandle(PathBuf);

impl SecretFileHandle {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

pub struct DirectTlsSecretHandles {
    pub certificate_chain: SecretFileHandle,
    pub private_key: SecretFileHandle,
}

enum LinuxIo {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ServerConnection, TcpStream>>),
}

impl LinuxIo {
    fn read(&mut self, destination: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(destination),
            Self::Tls(stream) => stream.read(destination),
        }
    }

    fn write(&mut self, source: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(source),
            Self::Tls(stream) => stream.write(source),
        }
    }

    fn close_notify(&mut self) {
        if let Self::Tls(stream) = self {
            stream.conn.send_close_notify();
            let _ = stream.flush();
        }
    }
}

struct LinuxConnection {
    io: LinuxIo,
    peer: IpAddr,
    buffer: Vec<u8>,
    sent: usize,
    pending_response: Vec<u8>,
    request_index: u64,
}

/// Real nonblocking Linux TCP/rustls backend. It parses only the bounded
/// HTTP/1.1 profile; frameworks may implement the same trait without changing
/// the semantic contracts.
pub struct LinuxHttpServingBackend {
    capabilities: HttpServingCapabilities,
    tls_config: Option<Arc<ServerConfig>>,
    listener: Option<TcpListener>,
    service: Option<OwnedServiceFacts>,
    connections: BTreeMap<u64, LinuxConnection>,
    next_connection: u64,
    evidence: VecDeque<HostedHttpEvidence>,
}

impl LinuxHttpServingBackend {
    pub fn new(
        capabilities: HttpServingCapabilities,
        tls: Option<DirectTlsSecretHandles>,
    ) -> Result<Self, HttpReason> {
        let tls_config = tls.map(load_server_config).transpose()?.map(Arc::new);
        Ok(Self {
            capabilities,
            tls_config,
            listener: None,
            service: None,
            connections: BTreeMap::new(),
            next_connection: 1,
            evidence: VecDeque::new(),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, HttpReason> {
        self.listener
            .as_ref()
            .ok_or(HttpReason::Closed)?
            .local_addr()
            .map_err(|_| HttpReason::IoFailure)
    }

    /// Stop admission while allowing already accepted requests to drain.
    pub fn begin_shutdown(&mut self) -> usize {
        self.listener = None;
        self.connections.len()
    }

    /// Cancel the bounded remainder after the plan's drain deadline.
    pub fn finish_shutdown(&mut self) -> usize {
        let remaining = self.connections.len();
        self.connections.clear();
        self.service = None;
        remaining
    }

    fn record(
        &mut self,
        exchange: Option<HttpExchangeId>,
        kind: HttpEvidenceKind,
        reason: Option<HttpReason>,
    ) {
        let Some(service) = self.service else {
            return;
        };
        if self.evidence.len() < usize::from(service.limits.maximum_evidence_events) {
            self.evidence.push_back(HostedHttpEvidence {
                service_identity: service.identity,
                exchange,
                kind,
                reason,
                security_mode: service.security_mode,
                encrypted: service.security_mode == HttpSecurityMode::DirectTls,
                proxy_authenticated: false,
                conduit_authority_checked: true,
            });
        }
    }

    fn ensure_evidence(&self, count: usize) -> Result<(), HttpReason> {
        let service = self.service.ok_or(HttpReason::Closed)?;
        if self
            .evidence
            .len()
            .checked_add(count)
            .is_none_or(|required| required > usize::from(service.limits.maximum_evidence_events))
        {
            Err(HttpReason::EvidenceFull)
        } else {
            Ok(())
        }
    }
}

fn load_server_config(handles: DirectTlsSecretHandles) -> Result<ServerConfig, HttpReason> {
    let certificate_file =
        File::open(handles.certificate_chain.0).map_err(|_| HttpReason::SecretHandleMissing)?;
    let certificates = rustls_pemfile::certs(&mut BufReader::new(certificate_file))
        .collect::<Result<Vec<CertificateDer<'static>>, _>>()
        .map_err(|_| HttpReason::CertificateInvalid)?;
    let key_file =
        File::open(handles.private_key.0).map_err(|_| HttpReason::SecretHandleMissing)?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(key_file))
        .map_err(|_| HttpReason::CertificateInvalid)?
        .ok_or(HttpReason::SecretHandleMissing)?;
    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .map_err(|_| HttpReason::CertificateInvalid)
}

impl HttpServingBackend for LinuxHttpServingBackend {
    type Connection = u64;
    type Error = HttpReason;
    type Evidence = HostedHttpEvidence;

    fn capabilities(&self) -> HttpServingCapabilities {
        self.capabilities
    }

    fn bind(
        &mut self,
        service: &ResolvedHttpService<'_>,
        authority: HttpServingAuthority<'_>,
    ) -> Result<(), Self::Error> {
        service.validate()?;
        authority.validate(service)?;
        if service.backend.id.as_str() != HTTP_LINUX_IMPLEMENTATION_ID {
            return Err(HttpReason::ImplementationMismatch);
        }
        if !self.capabilities.supports_security(service.security_mode) {
            return Err(HttpReason::UnsupportedSecurity);
        }
        if service.require_complete_stack_hard_bound
            && !self.capabilities.complete_stack_hard_bounded
        {
            return Err(HttpReason::ResourceUnderaccounted);
        }
        if service.security_mode == HttpSecurityMode::DirectTls && self.tls_config.is_none() {
            return Err(HttpReason::SecretHandleMissing);
        }
        if service.security_mode != HttpSecurityMode::DirectTls && self.tls_config.is_some() {
            return Err(HttpReason::SecurityBindingMismatch);
        }
        let listener = TcpListener::bind(service.listen).map_err(|_| HttpReason::IoFailure)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| HttpReason::IoFailure)?;
        self.listener = Some(listener);
        self.service = Some(OwnedServiceFacts {
            identity: service.identity,
            security_mode: service.security_mode,
            trusted_proxy: service.trusted_proxy,
            limits: service.limits,
        });
        self.record(None, HttpEvidenceKind::Bound, None);
        Ok(())
    }

    fn poll_accept(&mut self) -> Poll<Result<Self::Connection, Self::Error>> {
        let Some(listener) = &self.listener else {
            return Poll::Ready(Err(HttpReason::Closed));
        };
        let limits = match self.service {
            Some(service) => service.limits,
            None => return Poll::Ready(Err(HttpReason::Closed)),
        };
        if self.connections.len() >= usize::from(limits.maximum_connections) {
            return Poll::Ready(Err(HttpReason::AdmissionFull));
        }
        if let Err(error) = self.ensure_evidence(1) {
            return Poll::Ready(Err(error));
        }
        match listener.accept() {
            Ok((stream, peer)) => {
                if stream.set_nonblocking(true).is_err() {
                    return Poll::Ready(Err(HttpReason::IoFailure));
                }
                let id = self.next_connection;
                self.next_connection = self.next_connection.saturating_add(1);
                let io = if let Some(config) = &self.tls_config {
                    match ServerConnection::new(Arc::clone(config)) {
                        Ok(connection) => {
                            LinuxIo::Tls(Box::new(StreamOwned::new(connection, stream)))
                        }
                        Err(_) => return Poll::Ready(Err(HttpReason::CertificateInvalid)),
                    }
                } else {
                    LinuxIo::Plain(stream)
                };
                self.connections.insert(
                    id,
                    LinuxConnection {
                        io,
                        peer: peer.ip(),
                        buffer: Vec::with_capacity(limits.maximum_request_head_bytes as usize),
                        sent: 0,
                        pending_response: Vec::new(),
                        request_index: 1,
                    },
                );
                self.record(
                    Some(HttpExchangeId {
                        connection: id,
                        request: 1,
                    }),
                    HttpEvidenceKind::Accepted,
                    None,
                );
                Poll::Ready(Ok(id))
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Poll::Pending,
            Err(_) => Poll::Ready(Err(HttpReason::IoFailure)),
        }
    }

    fn poll_exchange(
        &mut self,
        connection: Self::Connection,
    ) -> Poll<Result<HttpExchangeEvent, Self::Error>> {
        let facts = match self.service {
            Some(service) => service,
            None => return Poll::Ready(Err(HttpReason::Closed)),
        };
        if let Err(error) = self.ensure_evidence(1) {
            return Poll::Ready(Err(error));
        }
        let Some(state) = self.connections.get_mut(&connection) else {
            return Poll::Ready(Err(HttpReason::Closed));
        };
        let ceiling = facts.limits.maximum_request_head_bytes.saturating_add(
            u32::try_from(facts.limits.maximum_request_body_bytes).unwrap_or(u32::MAX),
        ) as usize
            + 4;
        if state.buffer.len() >= ceiling {
            return Poll::Ready(Err(HttpReason::BodyTooLarge));
        }
        let mut scratch = [0_u8; 4096];
        match state.io.read(&mut scratch) {
            Ok(0) => return Poll::Ready(Err(HttpReason::Closed)),
            Ok(read) => state.buffer.extend_from_slice(&scratch[..read]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return Poll::Ready(Err(HttpReason::IoFailure)),
        }
        let Some(boundary) = find_header_boundary(&state.buffer) else {
            if state.buffer.len() as u32 > facts.limits.maximum_request_head_bytes {
                return Poll::Ready(Err(HttpReason::HeaderTooLarge));
            }
            return Poll::Pending;
        };
        let head = match std::str::from_utf8(&state.buffer[..boundary]) {
            Ok(head) => head,
            Err(_) => return Poll::Ready(Err(HttpReason::MalformedRequest)),
        };
        let content_length = head
            .split("\r\n")
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if content_length as u64 > facts.limits.maximum_request_body_bytes {
            return Poll::Ready(Err(HttpReason::BodyTooLarge));
        }
        if state.buffer.len() < boundary + 4 + content_length {
            return Poll::Pending;
        }
        let exchange = HttpExchangeId {
            connection,
            request: state.request_index,
        };
        let request = match parse_http_request(&state.buffer, exchange, state.peer, facts) {
            Ok(request) => request,
            Err(error) => return Poll::Ready(Err(error)),
        };
        state.buffer.clear();
        state.request_index = state.request_index.saturating_add(1);
        self.record(Some(exchange), HttpEvidenceKind::RequestReceived, None);
        Poll::Ready(Ok(HttpExchangeEvent::Request(request)))
    }

    fn poll_send(
        &mut self,
        connection: Self::Connection,
        response: &HttpResponsePart,
    ) -> Poll<Result<(), Self::Error>> {
        let facts = match self.service {
            Some(service) => service,
            None => return Poll::Ready(Err(HttpReason::Closed)),
        };
        if response.exchange.connection != connection {
            return Poll::Ready(Err(HttpReason::CorrelationMismatch));
        }
        if let Err(error) = validate_response(response, facts.limits) {
            return Poll::Ready(Err(error));
        }
        if let Err(error) = self.ensure_evidence(1) {
            return Poll::Ready(Err(error));
        }
        let Some(state) = self.connections.get_mut(&connection) else {
            return Poll::Ready(Err(HttpReason::Closed));
        };
        if state.pending_response.is_empty() {
            state.pending_response = encode_response(response);
            state.sent = 0;
        }
        match state.io.write(&state.pending_response[state.sent..]) {
            Ok(0) => Poll::Ready(Err(HttpReason::Closed)),
            Ok(written) => {
                state.sent += written;
                if state.sent == state.pending_response.len() {
                    state.pending_response.clear();
                    state.sent = 0;
                    self.record(
                        Some(response.exchange),
                        HttpEvidenceKind::ResponseSent,
                        None,
                    );
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Poll::Pending,
            Err(_) => Poll::Ready(Err(HttpReason::IoFailure)),
        }
    }

    fn cancel(
        &mut self,
        connection: Self::Connection,
        exchange: HttpExchangeId,
    ) -> Result<(), Self::Error> {
        self.ensure_evidence(1)?;
        if exchange.connection != connection {
            return Err(HttpReason::CorrelationMismatch);
        }
        self.connections
            .remove(&connection)
            .ok_or(HttpReason::Closed)?;
        self.record(
            Some(exchange),
            HttpEvidenceKind::Cancelled,
            Some(HttpReason::Cancelled),
        );
        Ok(())
    }

    fn close(&mut self, connection: Self::Connection) -> Result<(), Self::Error> {
        self.ensure_evidence(1)?;
        let mut connection = self
            .connections
            .remove(&connection)
            .ok_or(HttpReason::Closed)?;
        connection.io.close_notify();
        self.record(None, HttpEvidenceKind::Closed, None);
        Ok(())
    }

    fn take_evidence(&mut self) -> Option<Self::Evidence> {
        self.evidence.pop_front()
    }
}

/// Link the bounded Linux HTTP provider into an explicitly assembled hosted
/// registry. Merely publishing the semantic contract does not install it.
pub fn register_hosted_http_provider(registry: &mut Registry) -> Result<(), RegistryError> {
    static REQUIRED_AUTHORITIES: [SemanticHash; 1] = [SemanticHash::from_bytes([0x48; 32])];
    registry.register_compiled_in_host_service(CompiledInHostService {
        contract: &HTTP_SERVE_ONCE_CONTRACT,
        implementation_id: "conduit/http-linux-serve-once-v1",
        artifact_id: "conduit/http-linux-serve-once-artifact",
        entrypoint: "http-linux-serve-once",
        source_bytes: include_bytes!("lib.rs"),
        required_authorities: &REQUIRED_AUTHORITIES,
        factory: || Box::new(ServeOnceHandler),
        validate_config: validate_serve_once_config,
    })
}

struct ServeOnceHandler;

impl Handler for ServeOnceHandler {
    fn run(
        &mut self,
        node: &conduit_panel::Node,
        inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(RuntimeError::new(
                "CND-HTTP-004",
                "serve-once does not accept hidden value inputs",
            ));
        }
        validate_serve_once_config(node)
            .map_err(|error| RuntimeError::new(error.code, error.message))?;
        let listen = required_config(node, "listen")?;
        let path = required_config(node, "path")?;
        let response_body = required_config(node, "response")?.as_bytes().to_vec();
        let deadline_ms = required_config(node, "deadline_ms")?
            .parse::<u64>()
            .map_err(|_| RuntimeError::new("CND-HTTP-008", "invalid HTTP deadline"))?;

        let limits = HttpServiceLimits {
            maximum_request_head_bytes: 4096,
            maximum_request_body_bytes: 4096,
            maximum_response_bytes: 4096,
            maximum_header_count: 32,
            maximum_header_bytes: 2048,
            maximum_connections: 1,
            maximum_queued_admissions: 1,
            maximum_live_handlers: 1,
            maximum_sessions: 0,
            maximum_session_queue_items: 0,
            maximum_session_queue_bytes: 0,
            maximum_evidence_events: 16,
            header_deadline_ticks: deadline_ms,
            body_deadline_ticks: deadline_ms,
            handler_deadline_ticks: deadline_ms,
            drain_deadline_ticks: deadline_ms,
            reserved_memory_bytes: 32 * 1024,
        };
        let descriptor = |id, byte| PinnedDescriptor {
            id: Id(id),
            schema_version: 1,
            semantic_hash: SemanticHash::from_bytes([byte; 32]),
        };
        let mut service = ResolvedHttpService {
            identity: SemanticHash::from_bytes([0; 32]),
            service: descriptor("conduit.http/service", 1),
            backend: descriptor(HTTP_LINUX_IMPLEMENTATION_ID, 2),
            artifact: PlanArtifact {
                id: Id("conduit/http-linked-artifact"),
                digest: conduit_core::ArtifactDigest::from_bytes(
                    Sha256::digest(include_bytes!("lib.rs")).into(),
                ),
            },
            execution_profile: descriptor("conduit/http-bounded-once", 3),
            listen,
            protocol: HttpProtocol::Http11,
            security: descriptor("conduit.http/plaintext-explicit", 4),
            security_mode: HttpSecurityMode::Plaintext,
            certificate_identity: None,
            trusted_proxy: None,
            grant: Id("conduit.grant/http-loopback-listen"),
            secret_scope: None,
            require_complete_stack_hard_bound: false,
            limits,
        };
        service.identity = service.computed_identity();
        let capabilities = HttpServingCapabilities {
            profile_version: HTTP_PROFILE_VERSION,
            plaintext: true,
            direct_tls: false,
            trusted_proxy_tls: false,
            http11: true,
            http2: false,
            websocket: false,
            sse: false,
            maximum_request_head_bytes: limits.maximum_request_head_bytes,
            maximum_request_body_bytes: limits.maximum_request_body_bytes,
            maximum_response_bytes: limits.maximum_response_bytes,
            maximum_connections: limits.maximum_connections,
            maximum_sessions: 0,
            adapter_buffer_bytes: limits.reserved_memory_bytes,
            backend_buffer_bytes: 0,
            kernel_buffer_bytes: 0,
            complete_stack_hard_bounded: false,
        };
        let mut backend =
            LinuxHttpServingBackend::new(capabilities, None).map_err(http_runtime_error)?;
        // The exact executor has already validated the plan-pinned grant before
        // constructing this handler. This domain authority mirrors that
        // admitted decision at the backend boundary; it is not planner input.
        backend
            .bind(
                &service,
                HttpServingAuthority {
                    grant: service.grant,
                    allowed: true,
                    current_tick: 1,
                    valid_until_tick: 2,
                },
            )
            .map_err(http_runtime_error)?;
        let address = backend.local_addr().map_err(http_runtime_error)?;
        writeln!(io.error, "CND-HTTP-BOUND {address}")
            .and_then(|_| io.error.flush())
            .map_err(|_| RuntimeError::new("CND-HTTP-009", "cannot publish bound address"))?;

        let deadline = Instant::now() + Duration::from_millis(deadline_ms);
        let connection = poll_http_until(deadline, || backend.poll_accept())?;
        let request = match poll_http_until(deadline, || backend.poll_exchange(connection))? {
            HttpExchangeEvent::Request(request) => request,
            _ => {
                return Err(RuntimeError::new(
                    "CND-HTTP-010",
                    "HTTP exchange ended without a request",
                ));
            }
        };
        let route = HttpRoute {
            id: Id("route/checked-in"),
            order: 0,
            method: HttpMethod::Get,
            path_pattern: path,
        };
        let matched = match_route(&[route], request.head.method, &request.head.target)
            .map_err(http_runtime_error)?
            .is_some();
        let response = HttpResponsePart {
            exchange: request.exchange,
            status: if matched { 200 } else { 404 },
            headers: vec![HttpHeader {
                name: "content-type".to_owned(),
                value: "text/plain; charset=utf-8".to_owned(),
            }],
            body: if matched {
                response_body
            } else {
                b"not found\n".to_vec()
            },
            terminal: true,
        };
        poll_http_until(deadline, || backend.poll_send(connection, &response))?;
        backend.close(connection).map_err(http_runtime_error)?;
        Ok(Vec::new())
    }
}

fn poll_http_until<T>(
    deadline: Instant,
    mut operation: impl FnMut() -> Poll<Result<T, HttpReason>>,
) -> Result<T, RuntimeError> {
    loop {
        match operation() {
            Poll::Ready(result) => return result.map_err(http_runtime_error),
            Poll::Pending if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Poll::Pending => {
                return Err(RuntimeError::new(
                    HttpReason::Timeout.code(),
                    "bounded HTTP operation timed out",
                ));
            }
        }
    }
}

fn http_runtime_error(reason: HttpReason) -> RuntimeError {
    RuntimeError::new(reason.code(), reason.to_string())
}

fn required_config<'a>(node: &'a conduit_panel::Node, key: &str) -> Result<&'a str, RuntimeError> {
    node.config(key).ok_or_else(|| {
        RuntimeError::new(
            "CND-SRC-002",
            format!("HTTP service `{}` requires `{key}`", node.id),
        )
    })
}

fn validate_serve_once_config(node: &conduit_panel::Node) -> Result<(), ResolutionError> {
    let allowed = ["listen", "method", "path", "response", "deadline_ms"];
    if let Some(entry) = node
        .config
        .iter()
        .find(|entry| !allowed.contains(&entry.key.as_str()))
    {
        return Err(ResolutionError {
            code: "CND-SRC-002",
            message: format!("HTTP service has unknown field `{}`", entry.key),
        });
    }
    let value = |key| {
        node.config(key).ok_or_else(|| ResolutionError {
            code: "CND-SRC-002",
            message: format!("HTTP service requires `{key}`"),
        })
    };
    let listen = value("listen")?;
    let address = listen.parse::<SocketAddr>().map_err(|_| ResolutionError {
        code: "CND-HTTP-027",
        message: "HTTP listen address is invalid".to_owned(),
    })?;
    if !address.ip().is_loopback() || address.port() != 0 {
        return Err(ResolutionError {
            code: "CND-HTTP-027",
            message: "checked-in HTTP provider requires an ephemeral loopback address".to_owned(),
        });
    }
    if value("method")? != "GET" {
        return Err(ResolutionError {
            code: "CND-HTTP-010",
            message: "serve-once currently admits only GET".to_owned(),
        });
    }
    if !value("path")?.starts_with('/') {
        return Err(ResolutionError {
            code: "CND-HTTP-010",
            message: "HTTP route must be absolute".to_owned(),
        });
    }
    if value("response")?.len() > 4096 {
        return Err(ResolutionError {
            code: "CND-HTTP-013",
            message: "HTTP response exceeds the exact bound".to_owned(),
        });
    }
    let deadline = value("deadline_ms")?
        .parse::<u64>()
        .map_err(|_| ResolutionError {
            code: "CND-HTTP-008",
            message: "HTTP deadline is invalid".to_owned(),
        })?;
    if !(1..=30_000).contains(&deadline) {
        return Err(ResolutionError {
            code: "CND-HTTP-008",
            message: "HTTP deadline is outside the supported bound".to_owned(),
        });
    }
    Ok(())
}

fn encode_response(response: &HttpResponsePart) -> Vec<u8> {
    let reason = match response.status {
        101 => "Switching Protocols",
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Content Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Response",
    };
    let mut bytes = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\n",
        response.status,
        reason,
        response.body.len()
    )
    .into_bytes();
    for header in &response.headers {
        bytes.extend_from_slice(header.name.as_bytes());
        bytes.extend_from_slice(b": ");
        bytes.extend_from_slice(header.value.as_bytes());
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(if response.terminal {
        b"Connection: close\r\n\r\n"
    } else {
        b"\r\n"
    });
    bytes.extend_from_slice(&response.body);
    bytes
}

fn validate_response(
    response: &HttpResponsePart,
    limits: HttpServiceLimits,
) -> Result<(), HttpReason> {
    if response.body.len() as u64 > limits.maximum_response_bytes {
        return Err(HttpReason::ResponseTooLarge);
    }
    let header_bytes = response.headers.iter().try_fold(0_usize, |total, header| {
        if header.name.is_empty()
            || !header
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || header
                .value
                .bytes()
                .any(|byte| byte < b' ' && byte != b'\t')
        {
            return Err(HttpReason::MalformedRequest);
        }
        total
            .checked_add(header.name.len())
            .and_then(|total| total.checked_add(header.value.len()))
            .ok_or(HttpReason::HeaderTooLarge)
    })?;
    if response.headers.len() > usize::from(limits.maximum_header_count)
        || header_bytes as u32 > limits.maximum_header_bytes
    {
        return Err(HttpReason::HeaderTooLarge);
    }
    Ok(())
}

/// Security floors survive a serving-generation transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpTransition {
    Unchanged,
    DrainAndRebind,
}

pub fn validate_http_transition(
    old: &ResolvedHttpService<'_>,
    candidate: &ResolvedHttpService<'_>,
    overlap_memory_bytes: u64,
) -> Result<HttpTransition, HttpReason> {
    old.validate()?;
    candidate.validate()?;
    if old.security_mode != candidate.security_mode
        || old.protocol != candidate.protocol
        || old.trusted_proxy != candidate.trusted_proxy
    {
        return Err(HttpReason::UnsupportedSecurity);
    }
    let required = old
        .limits
        .reserved_memory_bytes
        .checked_add(candidate.limits.reserved_memory_bytes)
        .ok_or(HttpReason::ResourceUnderaccounted)?;
    if overlap_memory_bytes < required {
        return Err(HttpReason::ResourceUnderaccounted);
    }
    if old.identity == candidate.identity {
        Ok(HttpTransition::Unchanged)
    } else {
        Ok(HttpTransition::DrainAndRebind)
    }
}

#[cfg(test)]
mod tests {
    use super::{HttpMethod, HttpRoute, Id, match_route};

    #[test]
    fn route_order_is_explicit_and_parameterized() {
        let routes = [
            HttpRoute {
                id: Id("route/fallback"),
                order: 2,
                method: HttpMethod::Get,
                path_pattern: "/users/{id}",
            },
            HttpRoute {
                id: Id("route/first"),
                order: 1,
                method: HttpMethod::Get,
                path_pattern: "/users/{id}",
            },
        ];
        let matched = match_route(&routes, HttpMethod::Get, "/users/42?full=1")
            .unwrap()
            .unwrap();
        assert_eq!(matched.route, "route/first");
        assert_eq!(matched.parameters["id"], "42");
    }
}
