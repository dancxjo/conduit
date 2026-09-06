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
const SCRIPT: &[u8] = include_bytes!("../assets/app.js");
const FLOW_SCRIPT: &[u8] = include_bytes!("../assets/flow.js");
const FLOW_SCENE_SCRIPT: &[u8] = include_bytes!("../assets/flow-scene.js");
const FLOW_LAYOUT_SCRIPT: &[u8] = include_bytes!("../assets/flow-layout.js");
const FLOW_FACEPLATE_SCRIPT: &[u8] = include_bytes!("../assets/flow-faceplate.js");
const SHARED_PRESENTATION_SCRIPT: &[u8] = include_bytes!("../assets/shared-presentation.js");
const PORTABLE_NAVIGATION_SCRIPT: &[u8] = include_bytes!("../assets/portable-navigation.js");
const MEMBERSHIP_SCRIPT: &[u8] = include_bytes!("../assets/browser-membership.js");
const HOST_IDENTITY_SCRIPT: &[u8] =
    include_bytes!("../../../../targets/browser/host/assets/browser-host-identity.mjs");
const APPLICATION_PRESENTATION_SCRIPT: &[u8] =
    include_bytes!("../../../../targets/browser/host/assets/application-presentation.mjs");
const PRODUCT_MASTHEAD_SCRIPT: &[u8] =
    include_bytes!("../../../../semantics/presentation/assets/product-masthead.mjs");
const APPLICATION_THEME_SCRIPT: &[u8] =
    include_bytes!("../../../../targets/browser/host/assets/application-theme.mjs");
const APPLICATION_THEME_STYLE: &[u8] =
    include_bytes!("../../../../targets/browser/host/assets/application-theme.css");
const TEXT_LAB_RUNTIME_SCRIPT: &[u8] =
    include_bytes!("../../../../targets/browser/host/assets/text-lab-live-runtime.mjs");
const WEBSOCKET_LINE_SCRIPT: &[u8] =
    include_bytes!("../../../../targets/browser/host/assets/websocket-line.mjs");
const BODY_WEBRTC_SESSIONS_SCRIPT: &[u8] = include_bytes!("../assets/body-webrtc-sessions.mjs");
const BODY_WEBRTC_SESSION_SCRIPT: &[u8] = include_bytes!("../assets/body-webrtc-session.mjs");
const WEBRTC_LINE_SCRIPT: &[u8] = include_bytes!("../assets/webrtc-datachannel-line.mjs");
const WEBRTC_RUNTIME_SCRIPT: &[u8] = include_bytes!("../assets/webrtc-session-runtime.mjs");
const STYLE: &[u8] = include_bytes!("../assets/app.css");
const FLOW_STYLE: &[u8] = include_bytes!("../assets/flow.css");
const REACT: &[u8] = include_bytes!("../assets/react.min.js");
const REACT_DOM: &[u8] = include_bytes!("../assets/react-dom.min.js");
const REACT_FLOW: &[u8] = include_bytes!("../assets/react-flow.min.js");
const REACT_FLOW_STYLE: &[u8] = include_bytes!("../assets/react-flow.css");
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
        match path {
            "assets/app.js" => Some(SCRIPT),
            "assets/browser-membership.js" => Some(MEMBERSHIP_SCRIPT),
            "assets/browser-host-identity.mjs" => Some(HOST_IDENTITY_SCRIPT),
            "assets/body-webrtc-sessions.mjs" => Some(BODY_WEBRTC_SESSIONS_SCRIPT),
            "assets/body-webrtc-session.mjs" => Some(BODY_WEBRTC_SESSION_SCRIPT),
            "assets/webrtc-datachannel-line.mjs" => Some(WEBRTC_LINE_SCRIPT),
            "assets/webrtc-session-runtime.mjs" => Some(WEBRTC_RUNTIME_SCRIPT),
            "assets/application-presentation.mjs" => Some(APPLICATION_PRESENTATION_SCRIPT),
            "assets/product-masthead.mjs" => Some(PRODUCT_MASTHEAD_SCRIPT),
            "assets/application-theme.mjs" => Some(APPLICATION_THEME_SCRIPT),
            "assets/application-theme.css" => Some(APPLICATION_THEME_STYLE),
            "assets/flow.js" => Some(FLOW_SCRIPT),
            "assets/flow-scene.js" => Some(FLOW_SCENE_SCRIPT),
            "assets/flow-layout.js" => Some(FLOW_LAYOUT_SCRIPT),
            "assets/flow-faceplate.js" => Some(FLOW_FACEPLATE_SCRIPT),
            "assets/shared-presentation.js" => Some(SHARED_PRESENTATION_SCRIPT),
            "assets/portable-navigation.js" => Some(PORTABLE_NAVIGATION_SCRIPT),
            "assets/websocket-line.mjs" => Some(WEBSOCKET_LINE_SCRIPT),
            "assets/text-lab-live-runtime.mjs" => Some(TEXT_LAB_RUNTIME_SCRIPT),
            "assets/conduit-browser-runtime.wasm" => {
                Some(self.browser_wasm.as_deref().unwrap_or(EMPTY_BROWSER_WASM))
            }
            "assets/theme.css" => Some(&self.theme_css),
            "assets/app.css" => Some(STYLE),
            "assets/react-flow.css" => Some(REACT_FLOW_STYLE),
            "assets/flow.css" => Some(FLOW_STYLE),
            "assets/react.min.js" => Some(REACT),
            "assets/react-dom.min.js" => Some(REACT_DOM),
            "assets/react-flow.min.js" => Some(REACT_FLOW),
            _ => body_execution::assets::resource(path),
        }
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
        let (status, content_type, body): (&str, &str, &[u8]) = match first {
            "GET / HTTP/1.1" => ("200 OK", "text/html; charset=utf-8", INDEX),
            "GET /assets/app.js HTTP/1.1" => ("200 OK", "text/javascript; charset=utf-8", SCRIPT),
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
            "GET /assets/flow.js HTTP/1.1" => {
                ("200 OK", "text/javascript; charset=utf-8", FLOW_SCRIPT)
            }
            "GET /assets/flow-scene.js HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                FLOW_SCENE_SCRIPT,
            ),
            "GET /assets/flow-layout.js HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                FLOW_LAYOUT_SCRIPT,
            ),
            "GET /assets/flow-faceplate.js HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                FLOW_FACEPLATE_SCRIPT,
            ),
            "GET /assets/shared-presentation.js HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                SHARED_PRESENTATION_SCRIPT,
            ),
            "GET /assets/portable-navigation.js HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                PORTABLE_NAVIGATION_SCRIPT,
            ),
            "GET /assets/react.min.js HTTP/1.1" => {
                ("200 OK", "text/javascript; charset=utf-8", REACT)
            }
            "GET /assets/react-dom.min.js HTTP/1.1" => {
                ("200 OK", "text/javascript; charset=utf-8", REACT_DOM)
            }
            "GET /assets/react-flow.min.js HTTP/1.1" => {
                ("200 OK", "text/javascript; charset=utf-8", REACT_FLOW)
            }
            "GET /assets/browser-membership.js HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                MEMBERSHIP_SCRIPT,
            ),
            "GET /assets/browser-host-identity.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                HOST_IDENTITY_SCRIPT,
            ),
            "GET /assets/application-presentation.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_PRESENTATION_SCRIPT,
            ),
            "GET /assets/product-masthead.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                PRODUCT_MASTHEAD_SCRIPT,
            ),
            "GET /assets/application-theme.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_THEME_SCRIPT,
            ),
            "GET /assets/application-theme.css HTTP/1.1" => {
                ("200 OK", "text/css; charset=utf-8", APPLICATION_THEME_STYLE)
            }
            "GET /assets/text-lab-live-runtime.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                TEXT_LAB_RUNTIME_SCRIPT,
            ),
            "GET /assets/websocket-line.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                WEBSOCKET_LINE_SCRIPT,
            ),
            "GET /assets/body-webrtc-sessions.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                BODY_WEBRTC_SESSIONS_SCRIPT,
            ),
            "GET /assets/body-webrtc-session.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                BODY_WEBRTC_SESSION_SCRIPT,
            ),
            "GET /assets/webrtc-datachannel-line.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                WEBRTC_LINE_SCRIPT,
            ),
            "GET /assets/webrtc-session-runtime.mjs HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                WEBRTC_RUNTIME_SCRIPT,
            ),
            "GET /assets/conduit-browser-runtime.wasm HTTP/1.1" => self
                .browser_wasm
                .as_deref()
                .map_or(("200 OK", "application/wasm", EMPTY_BROWSER_WASM), |body| {
                    ("200 OK", "application/wasm", body)
                }),
            "GET /api/body-admission HTTP/1.1" => self.body_admission.as_deref().map_or(
                (
                    "404 Not Found",
                    "text/plain; charset=utf-8",
                    b"browser membership unavailable".as_slice(),
                ),
                |body| ("200 OK", "application/json; charset=utf-8", body),
            ),
            "GET /assets/app.css HTTP/1.1" => ("200 OK", "text/css; charset=utf-8", STYLE),
            "GET /assets/flow.css HTTP/1.1" => ("200 OK", "text/css; charset=utf-8", FLOW_STYLE),
            "GET /assets/react-flow.css HTTP/1.1" => {
                ("200 OK", "text/css; charset=utf-8", REACT_FLOW_STYLE)
            }
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
}

use navigation::navigation_state;
