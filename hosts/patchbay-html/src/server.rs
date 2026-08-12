use crate::{RendererSnapshot, SnapshotError};
use conduit_core::SignId;
use conduit_presentation::ManifestationFailure;
use patchbay_model::{
    InteractionDisposition, PatchbayAction, PatchbayEdit, PatchbayEditBasis, PatchbayInteraction,
    PatchbayInteractionRequest, PatchbayInvocationOutcome, PatchbayRefusal, PatchbaySubjectRef,
    PHOSPHOR_THEME,
};
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::time::Duration;

mod parts;
mod theme;

use theme::render_theme_css;

pub const MAX_HTTP_REQUEST_BYTES: usize = 8 * 1024;
pub const MAX_THEME_CSS_BYTES: usize = 2 * 1024;
const INDEX: &[u8] = include_bytes!("../assets/index.html");
const SCRIPT: &[u8] = include_bytes!("../assets/app.js");
const STYLE: &[u8] = include_bytes!("../assets/app.css");

#[derive(Debug)]
pub enum ServerError {
    Io(std::io::Error),
    Snapshot(SnapshotError),
    NonLoopbackBind,
    ThemeCssTooLarge,
    RequestTooLarge,
    InvalidRequest,
    Interaction(String),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Patchbay HTTP I/O error: {error}"),
            Self::Snapshot(error) => write!(f, "Patchbay snapshot error: {error}"),
            Self::NonLoopbackBind => f.write_str("Patchbay HTML binds only to IPv4 loopback"),
            Self::ThemeCssTooLarge => {
                f.write_str("Patchbay theme CSS exceeds its finite encoded bound")
            }
            Self::RequestTooLarge => f.write_str("Patchbay HTTP request exceeds its finite bound"),
            Self::InvalidRequest => f.write_str("Patchbay HTTP request is not valid UTF-8"),
            Self::Interaction(error) => write!(f, "Patchbay interaction error: {error}"),
        }
    }
}

impl std::error::Error for ServerError {}

#[derive(Debug)]
pub struct RendererDeliveryFailure {
    pub error: ServerError,
    pub snapshot: Box<RendererSnapshot>,
}

impl std::fmt::Display for RendererDeliveryFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "renderer delivery failed: {}", self.error)
    }
}

impl std::error::Error for RendererDeliveryFailure {}

impl From<std::io::Error> for ServerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<SnapshotError> for ServerError {
    fn from(value: SnapshotError) -> Self {
        Self::Snapshot(value)
    }
}

pub struct PatchbayHtmlServer {
    listener: TcpListener,
    snapshot: RendererSnapshot,
    encoded_snapshot: Vec<u8>,
    theme_css: Vec<u8>,
    interaction: PatchbayInteraction,
}

impl PatchbayHtmlServer {
    pub fn bind(address: SocketAddr, snapshot: &RendererSnapshot) -> Result<Self, ServerError> {
        if address.ip() != Ipv4Addr::LOCALHOST {
            return Err(ServerError::NonLoopbackBind);
        }
        let listener = TcpListener::bind(address)?;
        let mut snapshot = snapshot.clone();
        snapshot.mark_available(SignId::from("patchbay-html/document-ready"))?;
        let encoded_snapshot = snapshot.encode()?;
        let theme_css = render_theme_css(&PHOSPHOR_THEME);
        if theme_css.len() > MAX_THEME_CSS_BYTES {
            return Err(ServerError::ThemeCssTooLarge);
        }
        Ok(Self {
            listener,
            snapshot,
            encoded_snapshot,
            theme_css,
            interaction: PatchbayInteraction::new(
                conduit_core::HostId::from("patchbay-html/interaction-host"),
                conduit_core::BootId::from("patchbay-html/interaction-boot"),
            ),
        })
    }

    pub fn bind_ephemeral(snapshot: &RendererSnapshot) -> Result<Self, ServerError> {
        Self::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(), snapshot)
    }

    pub fn local_addr(&self) -> Result<SocketAddr, ServerError> {
        Ok(self.listener.local_addr()?)
    }

    pub fn serve(mut self) -> Result<(), RendererDeliveryFailure> {
        loop {
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) => return Err(self.delivery_failure(ServerError::Io(error))),
            };
            if let Err(error) = self.handle(stream) {
                return Err(self.delivery_failure(error));
            }
        }
    }

    pub fn serve_count(
        mut self,
        count: usize,
    ) -> Result<RendererSnapshot, RendererDeliveryFailure> {
        for _ in 0..count {
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) => return Err(self.delivery_failure(ServerError::Io(error))),
            };
            if let Err(error) = self.handle(stream) {
                return Err(self.delivery_failure(error));
            }
        }
        if let Err(error) = self
            .snapshot
            .mark_closed(SignId::from("patchbay-html/server-closed"))
        {
            return Err(self.delivery_failure(ServerError::Snapshot(error)));
        }
        Ok(self.snapshot)
    }

    fn delivery_failure(&mut self, error: ServerError) -> RendererDeliveryFailure {
        let _ = self.snapshot.mark_failed(
            ManifestationFailure::DeliveryFailed,
            SignId::from("patchbay-html/delivery-failed"),
        );
        RendererDeliveryFailure {
            error,
            snapshot: Box::new(self.snapshot.clone()),
        }
    }

    fn handle(&mut self, mut stream: TcpStream) -> Result<(), ServerError> {
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let request = match read_request(&mut stream) {
            Ok(request) => request,
            Err(ServerError::RequestTooLarge) => {
                return write_response(
                    &mut stream,
                    "413 Content Too Large",
                    "text/plain; charset=utf-8",
                    b"request too large",
                );
            }
            Err(ServerError::InvalidRequest) => {
                return write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    b"invalid request",
                );
            }
            Err(error) => return Err(error),
        };
        let first = request.head.split("\r\n").next().unwrap_or_default();
        if first == "POST /api/parts-interaction HTTP/1.1" {
            let body = match self.apply_parts_interaction(&request.body) {
                Ok(body) => body,
                Err(ServerError::InvalidRequest) => {
                    return write_response(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain; charset=utf-8",
                        b"invalid Parts interaction request",
                    );
                }
                Err(error) => return Err(error),
            };
            return write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                &body,
            );
        }
        if first == "POST /api/interaction HTTP/1.1" {
            let body = match self.apply_interaction(&request.body) {
                Ok(body) => body,
                Err(ServerError::InvalidRequest) => {
                    return write_response(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain; charset=utf-8",
                        b"invalid interaction request",
                    );
                }
                Err(error) => return Err(error),
            };
            return write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                &body,
            );
        }
        let (status, content_type, body): (&str, &str, &[u8]) = match first {
            "GET / HTTP/1.1" => ("200 OK", "text/html; charset=utf-8", INDEX),
            "GET /assets/app.js HTTP/1.1" => ("200 OK", "text/javascript; charset=utf-8", SCRIPT),
            "GET /assets/app.css HTTP/1.1" => ("200 OK", "text/css; charset=utf-8", STYLE),
            "GET /assets/theme.css HTTP/1.1" => (
                "200 OK",
                "text/css; charset=utf-8",
                self.theme_css.as_slice(),
            ),
            "GET /api/snapshot HTTP/1.1" => (
                "200 OK",
                "application/json; charset=utf-8",
                self.encoded_snapshot.as_slice(),
            ),
            _ if !first.starts_with("GET ") => (
                "405 Method Not Allowed",
                "text/plain; charset=utf-8",
                b"method not allowed".as_slice(),
            ),
            _ => (
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"not found".as_slice(),
            ),
        };
        write_response(&mut stream, status, content_type, body)
    }

    fn apply_interaction(&mut self, bytes: &[u8]) -> Result<Vec<u8>, ServerError> {
        let input: HtmlInteractionInput =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        let stale_presentation =
            input.presentation_id != self.snapshot.presentation.identity.as_str();
        let request_id = self
            .interaction
            .next_request_id(&input.kind)
            .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        let request = match input.kind.as_str() {
            "select" => PatchbayInteractionRequest::select(
                request_id,
                &PatchbaySubjectRef {
                    expanded_form_id: if stale_presentation {
                        conduit_core::ExpandedFormId::from(input.presentation_id.clone())
                    } else {
                        self.snapshot
                            .presentation
                            .basis
                            .expanded_form_id
                            .clone()
                            .ok_or(ServerError::InvalidRequest)?
                    },
                    subject_identity: input.subject.ok_or(ServerError::InvalidRequest)?,
                },
            ),
            "invoke" => PatchbayInteractionRequest::invoke(
                request_id,
                parse_html_action(input.action.as_deref().ok_or(ServerError::InvalidRequest)?)?,
                input.target.ok_or(ServerError::InvalidRequest)?,
            ),
            "edit" => PatchbayInteractionRequest::edit(
                request_id,
                parse_html_edit(input.edit.ok_or(ServerError::InvalidRequest)?)?,
            ),
            _ => return Err(ServerError::InvalidRequest),
        }
        .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        let expected_target = self
            .snapshot
            .presentation
            .basis
            .expanded_form_id
            .as_ref()
            .ok_or(ServerError::InvalidRequest)?
            .as_str()
            .to_owned();
        let presentation = self.snapshot.presentation.clone();
        let receipt = self
            .interaction
            .execute_presentation(&presentation, request, |request| match request {
                PatchbayInteractionRequest::Invoke { invocation, .. }
                    if stale_presentation || invocation.target_identity != expected_target =>
                {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation)
                }
                PatchbayInteractionRequest::Invoke { invocation, .. }
                    if invocation.action == PatchbayAction::ToggleLinearView =>
                {
                    PatchbayInvocationOutcome::Succeeded
                }
                PatchbayInteractionRequest::Edit { edit, .. }
                    if stale_presentation
                        || edit.basis().expanded_form_id.as_str() != expected_target =>
                {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation)
                }
                _ => PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable),
            })
            .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        self.snapshot.interaction.revision = self.snapshot.interaction.revision.saturating_add(1);
        if receipt.disposition == InteractionDisposition::Succeeded {
            if let PatchbayInteractionRequest::Select {
                subject_identity, ..
            } = &receipt.request
            {
                self.snapshot.interaction.selected_subject = Some(subject_identity.clone());
            }
        }
        self.snapshot.interaction.last_request_id =
            Some(receipt.request.request_id().as_str().into());
        self.snapshot.interaction.last_disposition = Some(format!("{:?}", receipt.disposition));
        self.snapshot.interaction.interaction_plan_id = Some(receipt.plan_id.as_str().into());
        self.snapshot.interaction.interaction_play_id =
            Some(receipt.active_play_id.as_str().into());
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HtmlInteractionInput {
    presentation_id: String,
    kind: String,
    subject: Option<String>,
    action: Option<String>,
    target: Option<String>,
    edit: Option<HtmlEditInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HtmlEditInput {
    source_document_id: String,
    source_revision: u64,
    expanded_form_id: String,
    operation: String,
    primary: String,
    secondary: Option<String>,
    key: Option<String>,
    value: Option<conduit_core::ConfigurationValue>,
}

fn parse_html_edit(input: HtmlEditInput) -> Result<PatchbayEdit, ServerError> {
    let basis = PatchbayEditBasis::new(
        conduit_core::SourceDocumentId::from(input.source_document_id),
        input.source_revision,
        conduit_core::ExpandedFormId::from(input.expanded_form_id),
    )
    .map_err(|_| ServerError::InvalidRequest)?;
    match input.operation.as_str() {
        "place-gear" => Ok(PatchbayEdit::PlaceGear {
            basis,
            kind_id: input.primary,
        }),
        "duplicate-gear" => Ok(PatchbayEdit::DuplicateGear {
            basis,
            subject_identity: input.primary,
        }),
        "remove-gear" => Ok(PatchbayEdit::RemoveGear {
            basis,
            subject_identity: input.primary,
        }),
        "remove-cord" => Ok(PatchbayEdit::RemoveCord {
            basis,
            subject_identity: input.primary,
        }),
        "connect-ports" => Ok(PatchbayEdit::ConnectPorts {
            basis,
            source_identity: input.primary,
            sink_identity: input.secondary.ok_or(ServerError::InvalidRequest)?,
        }),
        "reroute-cord" => Ok(PatchbayEdit::RerouteCord {
            basis,
            cord_identity: input.primary,
            endpoint_identity: input.secondary.ok_or(ServerError::InvalidRequest)?,
        }),
        "configure-gear" => Ok(PatchbayEdit::ConfigureGear {
            basis,
            subject_identity: input.primary,
            key: input.key.ok_or(ServerError::InvalidRequest)?,
            value: input.value.ok_or(ServerError::InvalidRequest)?,
        }),
        _ => Err(ServerError::InvalidRequest),
    }
}

fn parse_html_action(value: &str) -> Result<PatchbayAction, ServerError> {
    match value {
        "toggle-linear-view" => Ok(PatchbayAction::ToggleLinearView),
        _ => Err(ServerError::InvalidRequest),
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), ServerError> {
    write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
            body.len()
        )?;
    stream.write_all(body)?;
    Ok(())
}

struct HttpRequest {
    head: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, ServerError> {
    let mut bytes = Vec::with_capacity(1024);
    let mut chunk = [0u8; 512];
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_HTTP_REQUEST_BYTES {
            return Err(ServerError::RequestTooLarge);
        }
        if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let head_end = header_end + 4;
            let head = std::str::from_utf8(&bytes[..head_end])
                .map_err(|_| ServerError::InvalidRequest)?
                .to_owned();
            let content_length = head
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .or_else(|| line.strip_prefix("content-length: "))
                })
                .map(|value| value.parse::<usize>())
                .transpose()
                .map_err(|_| ServerError::InvalidRequest)?
                .unwrap_or(0);
            if head_end
                .checked_add(content_length)
                .is_none_or(|total| total > MAX_HTTP_REQUEST_BYTES)
            {
                return Err(ServerError::RequestTooLarge);
            }
            while bytes.len() < head_end + content_length {
                let count = stream.read(&mut chunk)?;
                if count == 0 {
                    return Err(ServerError::InvalidRequest);
                }
                bytes.extend_from_slice(&chunk[..count]);
            }
            return Ok(HttpRequest {
                head,
                body: bytes[head_end..head_end + content_length].to_vec(),
            });
        }
    }
    Err(ServerError::InvalidRequest)
}
