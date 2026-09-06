use crate::{RendererSnapshot, SnapshotError};
use conduit_browser_host::application_package;
use conduit_core::SignId;
use conduit_presentation::ManifestationFailure;
use patchbay_model::{PatchbayInteraction, CONDUIT_APPLICATION_THEME};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::time::Duration;

mod body_execution;
mod body_execution_proposal;
pub(crate) mod body_host_offer_evidence;
pub(crate) mod body_host_planning_offer;
mod body_membership_evidence;
mod body_routes;
mod body_workload;
mod browser_membership;
mod debug_control;
mod front_door;
mod http;
mod interaction;
mod navigation;
mod observation;
mod parts;
mod text_lab_loss;
mod timeline;
mod transition;
mod watches;

use crate::theme::render_theme_css;
use debug_control::DocumentaryDebuggerRuntime;
use http::{read_request, write_response};

pub const MAX_HTTP_REQUEST_BYTES: usize = 72 * 1024;
pub const MAX_THEME_CSS_BYTES: usize = 2 * 1024;
const INDEX: &[u8] = include_bytes!("../assets/index.html");
const APPLICATION_TEMPLATE: &[u8] = include_bytes!("../assets/patchbay.application.template.json");
const APPLICATION_LOADER: &[u8] =
    include_bytes!("../../../../targets/browser/host/assets/browser-application-loader.mjs");
const APPLICATION_STORAGE: &[u8] =
    include_bytes!("../../../../targets/browser/host/assets/browser-application-storage.mjs");
const MAX_BROWSER_WASM_BYTES: usize = 5 * 1024 * 1024;
const EMPTY_BROWSER_WASM: &[u8] = b"\0asm\x01\0\0\0";

#[derive(Debug)]
pub enum ServerError {
    Io(std::io::Error),
    Snapshot(SnapshotError),
    NonLoopbackBind,
    ThemeCssTooLarge,
    NavigationObservationTooLarge,
    RequestTooLarge,
    InvalidRequest,
    Interaction(String),
    ApplicationPackage(String),
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
            Self::NavigationObservationTooLarge => {
                f.write_str("Patchbay navigation observation exceeds its finite encoded bound")
            }
            Self::RequestTooLarge => f.write_str("Patchbay HTTP request exceeds its finite bound"),
            Self::InvalidRequest => f.write_str("Patchbay HTTP request is not valid UTF-8"),
            Self::Interaction(error) => write!(f, "Patchbay interaction error: {error}"),
            Self::ApplicationPackage(error) => {
                write!(f, "Patchbay application package error: {error}")
            }
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
    navigation: Option<conduit_presentation::NavigationState>,
    front_door: Option<std::sync::Arc<std::sync::Mutex<patchbay_model::LocalFrontDoor>>>,
    zero_body_front_door:
        Option<std::sync::Arc<std::sync::Mutex<patchbay_model::ZeroBodyFrontDoor>>>,
    body_workload: Option<patchbay_model::PatchbayBodyWorkloadSession>,
    body_planning_forms: Vec<patchbay_model::FormCandidate>,
    body_planning: Option<patchbay_model::BodyPlanningSession>,
    body_admission: Option<Vec<u8>>,
    browser_wasm: Option<Vec<u8>>,
    text_lab_base: Option<String>,
    debug_runtime: Option<DocumentaryDebuggerRuntime>,
}

impl PatchbayHtmlServer {
    fn application_resource(&self, path: &str) -> Option<&[u8]> {
        crate::application_resources::resource(
            path,
            self.browser_wasm.as_deref().unwrap_or(EMPTY_BROWSER_WASM),
            &self.theme_css,
        )
        .map(|(_, bytes)| bytes)
    }

    fn application_manifest(&self) -> Result<Vec<u8>, ServerError> {
        application_package::build_manifest(APPLICATION_TEMPLATE, |path| {
            self.application_resource(path)
        })
        .map_err(ServerError::ApplicationPackage)
    }

    pub fn bind(address: SocketAddr, snapshot: &RendererSnapshot) -> Result<Self, ServerError> {
        if address.ip() != Ipv4Addr::LOCALHOST {
            return Err(ServerError::NonLoopbackBind);
        }
        let listener = TcpListener::bind(address)?;
        let mut snapshot = snapshot.clone();
        snapshot.mark_available(SignId::from("patchbay-html/document-ready"))?;
        let encoded_snapshot = snapshot.encode()?;
        let navigation = navigation_state(&snapshot)?;
        let theme_css = render_theme_css(&CONDUIT_APPLICATION_THEME);
        if theme_css.len() > MAX_THEME_CSS_BYTES {
            return Err(ServerError::ThemeCssTooLarge);
        }
        let debug_runtime = DocumentaryDebuggerRuntime::from_snapshot(&snapshot)?;
        let body_workload = body_workload::open(&snapshot)?;
        Ok(Self {
            listener,
            snapshot,
            encoded_snapshot,
            theme_css,
            interaction: PatchbayInteraction::new(
                conduit_core::HostId::from("patchbay-html/interaction-host"),
                conduit_core::BootId::from("patchbay-html/interaction-boot"),
            ),
            navigation,
            front_door: None,
            zero_body_front_door: None,
            body_workload,
            body_planning_forms: Vec::new(),
            body_planning: None,
            body_admission: None,
            browser_wasm: None,
            text_lab_base: None,
            debug_runtime,
        })
    }

    pub fn bind_ephemeral(snapshot: &RendererSnapshot) -> Result<Self, ServerError> {
        Self::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(), snapshot)
    }

    pub fn with_body_planning_forms(
        mut self,
        forms: Vec<patchbay_model::FormCandidate>,
    ) -> Result<Self, ServerError> {
        let workset = &self
            .body_workload
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("Body workload session is absent".into()))?
            .evidence()
            .body
            .workset;
        patchbay_model::body_planning_requirements(workset, &forms)
            .map_err(|error| ServerError::Interaction(format!("Body planning forms: {error:?}")))?;
        self.body_planning_forms = forms;
        Ok(self)
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
        if first == "POST /api/text-lab-loss HTTP/1.1" {
            let body = match self.apply_text_lab_loss(&request.body) {
                Ok(body) => body,
                Err(ServerError::InvalidRequest | ServerError::Interaction(_)) => {
                    return write_response(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain; charset=utf-8",
                        b"invalid Text Lab loss receipt",
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
        if first == "GET /api/text-lab-base HTTP/1.1" {
            let base = self
                .text_lab_base
                .as_ref()
                .ok_or(ServerError::InvalidRequest)?;
            let body = serde_json::to_vec(&serde_json::json!({ "base": base }))
                .map_err(|error| ServerError::Interaction(error.to_string()))?;
            return write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                &body,
            );
        }
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
        if let Some(result) = self.deliver_body_route(first, &mut stream, &request.body) {
            return result;
        }
        if first == "POST /api/front-door-transition HTTP/1.1" {
            let body = self.apply_front_door_transition(&request.body)?;
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
        if first == "POST /api/debugger-watch HTTP/1.1" {
            let body = match self.apply_debugger_watch(&request.body) {
                Ok(body) => body,
                Err(ServerError::InvalidRequest | ServerError::Interaction(_)) => {
                    return write_response(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain; charset=utf-8",
                        b"invalid debugger Watch request",
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
        if first == "POST /api/debugger-timeline HTTP/1.1" {
            let body = match self.apply_debugger_timeline(&request.body) {
                Ok(body) => body,
                Err(ServerError::InvalidRequest | ServerError::Interaction(_)) => {
                    return write_response(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain; charset=utf-8",
                        b"invalid debugger timeline request",
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
        if first == "POST /api/debugger-control HTTP/1.1" {
            let body = match self.apply_debugger_control(&request.body) {
                Ok(body) => body,
                Err(ServerError::InvalidRequest | ServerError::Interaction(_)) => {
                    return write_response(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain; charset=utf-8",
                        b"invalid debugger control request",
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
        if first == "POST /api/navigation HTTP/1.1" {
            let body = match self.apply_navigation(&request.body) {
                Ok(body) => body,
                Err(ServerError::InvalidRequest) => {
                    return write_response(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain; charset=utf-8",
                        b"invalid navigation request",
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
        if matches!(
            first,
            "GET /api/snapshot HTTP/1.1" | "GET /api/navigation-observation HTTP/1.1"
        ) && (self.front_door.is_some() || self.zero_body_front_door.is_some())
        {
            self.refresh_front_door()?;
        }
        if first == "GET /api/navigation-observation HTTP/1.1" {
            return self.write_navigation_observation(&mut stream);
        }
        if first == "GET /patchbay.application.json HTTP/1.1" {
            let manifest = self.application_manifest()?;
            return write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                &manifest,
            );
        }
        if let Some(path) = first
            .strip_prefix("GET /")
            .and_then(|line| line.strip_suffix(" HTTP/1.1"))
        {
            if let Some((media_type, bytes)) = crate::application_resources::resource(
                path,
                self.browser_wasm.as_deref().unwrap_or(EMPTY_BROWSER_WASM),
                &self.theme_css,
            ) {
                return write_response(&mut stream, "200 OK", media_type, bytes);
            }
        }
        let (status, content_type, body): (&str, &str, &[u8]) = match first {
            "GET / HTTP/1.1" => ("200 OK", "text/html; charset=utf-8", INDEX),
            "GET /assets/browser-application-loader.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_LOADER,
            ),
            "GET /assets/browser-application-storage.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_STORAGE,
            ),
            "GET /api/body-admission HTTP/1.1" => self.body_admission.as_deref().map_or(
                (
                    "404 Not Found",
                    "text/plain; charset=utf-8",
                    b"browser membership unavailable".as_slice(),
                ),
                |body| ("200 OK", "application/json; charset=utf-8", body),
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
}

use navigation::navigation_state;
