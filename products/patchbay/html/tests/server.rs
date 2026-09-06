use conduit_presentation::{ManifestationFailure, ManifestationLifecycle};
use patchbay_html::{demonstration_snapshot, PatchbayHtmlServer, ServerError};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};

fn request(path: &str, method: &str) -> String {
    let snapshot = demonstration_snapshot().unwrap();
    let server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || server.serve_count(1));
    let mut stream = TcpStream::connect(address).unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let closed = worker.join().unwrap().unwrap();
    assert_eq!(
        closed.renderer.manifestation.lifecycle,
        ManifestationLifecycle::Closed
    );
    response
}

fn post_interaction(snapshot: patchbay_html::RendererSnapshot, body: &[u8]) -> String {
    let server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || server.serve_count(1));
    let mut stream = TcpStream::connect(address).unwrap();
    write!(
        stream,
        "POST /api/interaction HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    worker.join().unwrap().unwrap();
    response
}

fn post_parts_interaction(snapshot: patchbay_html::RendererSnapshot, body: &[u8]) -> String {
    let server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || server.serve_count(1));
    let mut stream = TcpStream::connect(address).unwrap();
    write!(
        stream,
        "POST /api/parts-interaction HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    worker.join().unwrap().unwrap();
    response
}

fn post_debugger_watch(snapshot: patchbay_html::RendererSnapshot, body: &[u8]) -> String {
    let server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || server.serve_count(1));
    let mut stream = TcpStream::connect(address).unwrap();
    write!(
        stream,
        "POST /api/debugger-watch HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    worker.join().unwrap().unwrap();
    response
}

fn post_debugger_timeline(snapshot: patchbay_html::RendererSnapshot, body: &[u8]) -> String {
    let server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || server.serve_count(1));
    let mut stream = TcpStream::connect(address).unwrap();
    write!(
        stream,
        "POST /api/debugger-timeline HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    worker.join().unwrap().unwrap();
    response
}

fn post_to(address: std::net::SocketAddr, path: &str, body: &[u8]) -> String {
    let mut stream = TcpStream::connect(address).unwrap();
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn debugger_timeline_pause_and_event_selection_mutate_only_playback_state() {
    let snapshot = demonstration_snapshot().unwrap();
    let presentation = snapshot.presentation.clone();
    let timeline_revision = snapshot.timeline.as_ref().unwrap().revision;
    let response = post_debugger_timeline(
        snapshot,
        &serde_json::to_vec(&serde_json::json!({
            "presentation_id": presentation.identity.as_str(),
            "presentation_revision": presentation.revision,
            "timeline_revision": timeline_revision,
            "action": "select-event",
            "index": 0,
        }))
        .unwrap(),
    );
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    let selected = patchbay_html::RendererSnapshot::decode(
        response.split("\r\n\r\n").nth(1).unwrap().as_bytes(),
        presentation.revision,
    )
    .unwrap();
    assert_eq!(selected.presentation, presentation);
    assert_eq!(selected.timeline.as_ref().unwrap().selected_event, Some(0));
    assert_eq!(
        selected
            .timeline_projection
            .as_ref()
            .unwrap()
            .cursor_sequence,
        Some(39)
    );
    assert_eq!(
        selected.timeline_projection.as_ref().unwrap().mode,
        patchbay_model::DebuggerTimelineMode::Replay
    );
}

#[test]
fn debugger_trace_uses_exact_causal_parent_and_not_graph_reachability() {
    let snapshot = demonstration_snapshot().unwrap();
    let presentation = snapshot.presentation.clone();
    let response = post_debugger_timeline(
        snapshot.clone(),
        &serde_json::to_vec(&serde_json::json!({
            "presentation_id": presentation.identity.as_str(),
            "presentation_revision": presentation.revision,
            "timeline_revision": snapshot.timeline.as_ref().unwrap().revision,
            "action": "trace-upstream",
            "index": 1,
        }))
        .unwrap(),
    );
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    let traced = patchbay_html::RendererSnapshot::decode(
        response.split("\r\n\r\n").nth(1).unwrap().as_bytes(),
        presentation.revision,
    )
    .unwrap();
    assert_eq!(
        traced
            .timeline
            .as_ref()
            .unwrap()
            .trace
            .as_ref()
            .unwrap()
            .steps
            .iter()
            .map(|step| step.sequence)
            .collect::<Vec<_>>(),
        vec![39, 40]
    );
    assert_eq!(traced.presentation, presentation);
}

#[test]
fn debugger_break_and_resume_cross_the_real_kernel_control_contract() {
    let snapshot = demonstration_snapshot().unwrap();
    let presentation = snapshot.presentation.clone();
    let control = snapshot.debugger_control.as_ref().unwrap();
    let subject = control.eligible_subjects[0].clone();
    let server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || server.serve_count(2));
    let suspended_response = post_to(
        address,
        "/api/debugger-control",
        &serde_json::to_vec(&serde_json::json!({
            "presentation_id": presentation.identity.as_str(),
            "presentation_revision": presentation.revision,
            "control_revision": control.revision,
            "action": "break-here",
            "subject": subject,
        }))
        .unwrap(),
    );
    let suspended = patchbay_html::RendererSnapshot::decode(
        suspended_response
            .split("\r\n\r\n")
            .nth(1)
            .unwrap()
            .as_bytes(),
        presentation.revision,
    )
    .unwrap();
    assert_eq!(
        suspended.debugger_control.as_ref().unwrap().state,
        patchbay_model::DebuggerExecutionControlState::Suspended
    );
    assert_eq!(suspended.presentation, presentation);
    let resumed_response = post_to(
        address,
        "/api/debugger-control",
        &serde_json::to_vec(&serde_json::json!({
            "presentation_id": presentation.identity.as_str(),
            "presentation_revision": presentation.revision,
            "control_revision": suspended.debugger_control.as_ref().unwrap().revision,
            "action": "resume",
        }))
        .unwrap(),
    );
    let resumed = patchbay_html::RendererSnapshot::decode(
        resumed_response
            .split("\r\n\r\n")
            .nth(1)
            .unwrap()
            .as_bytes(),
        presentation.revision,
    )
    .unwrap();
    assert_eq!(
        resumed.debugger_control.as_ref().unwrap().state,
        patchbay_model::DebuggerExecutionControlState::Running
    );
    assert_eq!(resumed.presentation, presentation);
    worker.join().unwrap().unwrap();
}

#[test]
fn debugger_watch_mutation_is_exact_revisioned_and_topology_immutable() {
    let snapshot = demonstration_snapshot().unwrap();
    let presentation = snapshot.presentation.clone();
    let watches = snapshot.watches.as_ref().unwrap();
    let subject = watches
        .eligible_subjects
        .iter()
        .find(|(_, role)| *role == patchbay_model::DebuggerWatchSubjectRole::Cord)
        .unwrap()
        .0
        .clone();
    let body = serde_json::to_vec(&serde_json::json!({
        "presentation_id": presentation.identity.as_str(),
        "presentation_revision": presentation.revision,
        "watch_revision": watches.revision,
        "action": "add",
        "subject": subject,
    }))
    .unwrap();
    let response = post_debugger_watch(snapshot.clone(), &body);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    let watched = patchbay_html::RendererSnapshot::decode(
        response.split("\r\n\r\n").nth(1).unwrap().as_bytes(),
        presentation.revision,
    )
    .unwrap();
    assert_eq!(watched.presentation, presentation);
    let watch = &watched.watches.as_ref().unwrap().watches[0];
    assert_eq!(watch.subject, subject);
    assert_eq!(watch.latest.as_ref().unwrap().sequence, 42);
    assert_eq!(watch.telemetry_gap.as_ref().unwrap().dropped_records, 2);

    let stale = post_debugger_watch(
        snapshot,
        &serde_json::to_vec(&serde_json::json!({
            "presentation_id": presentation.identity.as_str(),
            "presentation_revision": presentation.revision,
            "watch_revision": 99,
            "action": "add",
            "subject": subject,
        }))
        .unwrap(),
    );
    assert!(stale.starts_with("HTTP/1.1 400 Bad Request"));
}

#[test]
fn exact_read_only_routes_are_bounded_no_store_and_typed() {
    let index = request("/", "GET");
    assert!(index.starts_with("HTTP/1.1 200 OK"));
    assert!(index.contains("Cache-Control: no-store"));
    assert!(index.contains("X-Content-Type-Options: nosniff"));
    assert!(index.contains("Content-Security-Policy: default-src 'self'"));
    assert!(index.contains("Entrance choices"));
    assert!(index.contains("Here and membership"));
    assert!(index.contains("Wants to join"));
    assert!(index.contains("Exact truth and accessibility"));
    let package = request("/patchbay.application.json", "GET");
    assert!(package.starts_with("HTTP/1.1 200 OK"));
    let manifest: serde_json::Value =
        serde_json::from_str(package.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(manifest["application_id"], "conduit.application/patchbay");
    assert_eq!(manifest["resources"].as_array().unwrap().len(), 32);
    assert!(manifest["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| {
            resource["role"] == "body-plan-inspection"
                && resource["path"] == "assets/body-plan-inspection.mjs"
                && resource["kind"] == "module"
        }));
    assert!(manifest["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| {
            resource["role"] == "shared-presentation"
                && resource["path"] == "assets/shared-presentation.js"
                && resource["kind"] == "module"
        }));
    assert!(manifest["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| {
            resource["role"] == "product-masthead"
                && resource["path"] == "assets/product-masthead.mjs"
                && resource["kind"] == "module"
        }));
    let shared_presentation = request("/assets/shared-presentation.js", "GET");
    assert!(shared_presentation.starts_with("HTTP/1.1 200 OK"));
    assert!(shared_presentation.contains("createPatchbaySharedPresentation"));
    let navigation = request("/assets/portable-navigation.js", "GET");
    assert!(navigation.starts_with("HTTP/1.1 200 OK"));
    assert!(navigation.contains("projectCurrent"));

    let response = request("/api/snapshot", "GET");
    let body = response.split("\r\n\r\n").nth(1).unwrap();
    let decoded = patchbay_html::RendererSnapshot::decode(body.as_bytes(), 1).unwrap();
    assert_eq!(decoded.revision, 1);
    let parts = decoded.parts.as_ref().unwrap();
    assert_eq!(parts.parts.len(), 3);
    assert_eq!(parts.wants_to_join.len(), 1);
    assert!(parts.new_realization_possibilities);
    assert_eq!(
        decoded.renderer.manifestation.lifecycle,
        ManifestationLifecycle::Available
    );
    let observation = request("/api/navigation-observation", "GET");
    assert!(observation.starts_with("HTTP/1.1 200 OK"));
    let observed: conduit_presentation::NavigationObservation =
        serde_json::from_str(observation.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(Some(observed), decoded.navigation_observation().unwrap());
    assert_eq!(
        request("/unknown", "GET").lines().next(),
        Some("HTTP/1.1 404 Not Found")
    );
    assert_eq!(
        request("/api/snapshot", "POST").lines().next(),
        Some("HTTP/1.1 405 Method Not Allowed")
    );
}

#[test]
fn product_serves_one_canonical_bounded_webrtc_client_module_graph() {
    for (path, marker) in [
        ("/assets/browser-membership.js", "BodyWebRtcSessions"),
        (
            "/assets/browser-host-identity.mjs",
            "openBrowserHostIdentity",
        ),
        (
            "/assets/application-presentation.mjs",
            "createApplicationPresentationHost",
        ),
        ("/assets/product-masthead.mjs", "createProductMasthead"),
        ("/assets/application-theme.mjs", "applicationThemeLimits"),
        (
            "/assets/body-webrtc-sessions.mjs",
            "class BodyWebRtcSessions",
        ),
        ("/assets/body-webrtc-session.mjs", "class BodyWebRtcSession"),
        (
            "/assets/webrtc-datachannel-line.mjs",
            "class BrowserWebRtcDataChannelLine",
        ),
        (
            "/assets/webrtc-session-runtime.mjs",
            "instantiateGrantedWebRtcSession",
        ),
    ] {
        let response = request(path, "GET");
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{path}");
        assert!(response.contains("Content-Type: text/javascript; charset=utf-8"));
        assert!(response.contains("Cache-Control: no-store"));
        assert!(response.contains(marker), "{path}");
    }
}

#[test]
fn parts_inspection_is_non_mutating_and_ambient_admission_is_never_implicit() {
    let snapshot = demonstration_snapshot().unwrap();
    let presentation_id = snapshot.presentation.identity.as_str().to_owned();
    let parts = snapshot.parts.as_ref().unwrap();
    let body_id = parts.body_id.as_str().to_owned();
    let candidate = parts.wants_to_join[0].candidate_id.as_str().to_owned();
    let form = snapshot.presentation.basis.checked_form_id.clone();
    let plan = snapshot.presentation.basis.plan_id.clone();
    let response = post_parts_interaction(
        snapshot,
        &serde_json::to_vec(&serde_json::json!({
            "presentation_id": presentation_id,
            "body_id": body_id,
            "action": "Inspect",
            "target": candidate,
        }))
        .unwrap(),
    );
    let decoded = patchbay_html::RendererSnapshot::decode(
        response.split("\r\n\r\n").nth(1).unwrap().as_bytes(),
        1,
    )
    .unwrap();
    assert_eq!(
        decoded.interaction.parts_disposition.as_deref(),
        Some("Succeeded")
    );
    assert_eq!(
        decoded.interaction.selected_candidate.as_deref(),
        Some(candidate.as_str())
    );
    assert!(decoded.interaction.selected_part.is_none());
    assert_eq!(decoded.presentation.basis.checked_form_id, form);
    assert_eq!(decoded.presentation.basis.plan_id, plan);
    assert_eq!(decoded.parts.unwrap().wants_to_join.len(), 1);
}

#[test]
fn parts_mutation_without_a_coordinator_refuses_nonfatally_and_stale_basis_refuses_first() {
    let snapshot = demonstration_snapshot().unwrap();
    let parts = snapshot.parts.as_ref().unwrap();
    let body_id = parts.body_id.as_str().to_owned();
    let candidate = parts.wants_to_join[0].candidate_id.as_str().to_owned();
    let response = post_parts_interaction(
        snapshot.clone(),
        &serde_json::to_vec(&serde_json::json!({
            "presentation_id": snapshot.presentation.identity,
            "body_id": body_id,
            "action": "Admit",
            "target": candidate,
        }))
        .unwrap(),
    );
    let decoded = patchbay_html::RendererSnapshot::decode(
        response.split("\r\n\r\n").nth(1).unwrap().as_bytes(),
        1,
    )
    .unwrap();
    assert_eq!(
        decoded.interaction.parts_disposition.as_deref(),
        Some("Refused")
    );
    assert!(decoded
        .interaction
        .parts_feedback
        .as_deref()
        .unwrap()
        .contains("no attached Body coordinator"));
    assert_eq!(decoded.parts.unwrap().wants_to_join.len(), 1);

    let stale = post_parts_interaction(
        snapshot,
        br#"{"presentation_id":"stale","body_id":"stale","action":"Inspect","target":"stale"}"#,
    );
    let decoded = patchbay_html::RendererSnapshot::decode(
        stale.split("\r\n\r\n").nth(1).unwrap().as_bytes(),
        1,
    )
    .unwrap();
    assert!(decoded
        .interaction
        .parts_feedback
        .as_deref()
        .unwrap()
        .contains("basis is stale"));
}

#[test]
fn html_theme_sheet_maps_the_shared_identity_and_every_bounded_token() {
    let response = request("/assets/theme.css", "GET");
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("Content-Type: text/css; charset=utf-8"));
    let body = response.split("\r\n\r\n").nth(1).unwrap();
    assert!(body.len() <= patchbay_html::MAX_THEME_CSS_BYTES);
    assert!(body.contains("--conduit-theme-identity:\"conduit.presentation/phosphor@1\""));
    assert!(!body.contains("--patchbay-"));
    for (token, color) in [
        ("background", "#05070B"),
        ("surface", "#090D16"),
        ("structure-primary", "#0DD8F6"),
        ("structure-secondary", "#0A1F87"),
        ("text-primary", "#93D2F7"),
        ("text-secondary", "#578EC9"),
        ("emphasis", "#E9A325"),
        ("focus", "#F4C400"),
    ] {
        assert!(body.contains(&format!("--conduit-{token}:{color}")));
    }
    assert!(body.contains("--conduit-type-body:16px"));
    assert!(body.contains("--conduit-line-height:150%"));
    assert!(body.contains("--conduit-space-panel:13px"));
    assert!(body.contains("--conduit-radius-panel:9px"));
    assert!(body.contains("--conduit-responsive-breakpoint:720px"));

    let application = request("/assets/app.css", "GET");
    assert!(application.contains("var(--conduit-background)"));
    assert!(application.contains("var(--conduit-focus)"));
    assert!(!application.contains("#08111f"));

    let shared = request("/assets/application-theme.css", "GET");
    assert!(shared.starts_with("HTTP/1.1 200 OK"));
    assert!(shared.contains("[data-application-component=\"navigation\"]"));

    let flow = request("/assets/flow.css", "GET");
    assert!(flow.contains(".flow-faceplate"));
    assert!(flow.contains(".react-flow__edge.animated"));
    assert!(flow.contains("@media (prefers-reduced-motion: reduce)"));
    assert!(!application.contains(".flow-faceplate header"));
}

#[test]
fn server_rejects_non_loopback_exposure() {
    let snapshot = demonstration_snapshot().unwrap();
    assert!(matches!(
        PatchbayHtmlServer::bind(
            SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into(),
            &snapshot
        ),
        Err(ServerError::NonLoopbackBind)
    ));
}

#[test]
fn structured_browser_edit_normalizes_without_dom_or_widget_identity() {
    let snapshot = demonstration_snapshot().unwrap();
    let presentation_id = snapshot.presentation.identity.as_str();
    let basis = &snapshot.presentation.basis;
    let expanded = basis.expanded_form_id.as_ref().unwrap().as_str();
    let body = serde_json::to_vec(&serde_json::json!({
        "presentation_id": presentation_id,
        "presentation_revision": snapshot.presentation.revision,
        "kind": "edit",
        "subject": null,
        "action_id": null,
        "edit": {
            "source_document_id": basis.source_document_id.as_ref().unwrap().as_str(),
            "source_revision": 7,
            "expanded_form_id": expanded,
            "operation": "configure-gear",
            "primary": "gear/count-demo/counter",
            "secondary": null,
            "key": "label",
            "value": {"Text": "literal@delimiter:is-data"}
        }
    }))
    .unwrap();
    let response = post_interaction(snapshot, &body);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    let decoded = patchbay_html::RendererSnapshot::decode(
        response.split("\r\n\r\n").nth(1).unwrap().as_bytes(),
        1,
    )
    .unwrap();
    assert_eq!(
        decoded.interaction.last_disposition.as_deref(),
        Some("Refused(OperationUnavailable)")
    );
    assert!(decoded
        .interaction
        .last_request_id
        .as_deref()
        .is_some_and(|id| id.contains("/edit/")));
}

#[test]
fn malformed_and_oversized_requests_do_not_stop_later_delivery() {
    let snapshot = demonstration_snapshot().unwrap();
    let server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || server.serve_count(3));

    let mut malformed = TcpStream::connect(address).unwrap();
    malformed.write_all(b"GET /\xff HTTP/1.1\r\n\r\n").unwrap();
    let mut malformed_response = String::new();
    malformed.read_to_string(&mut malformed_response).unwrap();
    assert!(malformed_response.starts_with("HTTP/1.1 400 Bad Request"));

    let mut oversized = TcpStream::connect(address).unwrap();
    oversized
        .write_all(&vec![b'x'; patchbay_html::MAX_HTTP_REQUEST_BYTES + 1])
        .unwrap();
    let mut oversized_response = String::new();
    oversized.read_to_string(&mut oversized_response).unwrap();
    assert!(oversized_response.starts_with("HTTP/1.1 413 Content Too Large"));

    let mut valid = TcpStream::connect(address).unwrap();
    valid
        .write_all(b"GET /api/snapshot HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut valid_response = String::new();
    valid.read_to_string(&mut valid_response).unwrap();
    assert!(valid_response.starts_with("HTTP/1.1 200 OK"));

    worker.join().unwrap().unwrap();
}

#[test]
fn transport_failure_yields_an_exact_failed_manifestation_sign() {
    let snapshot = demonstration_snapshot().unwrap();
    let source_identity = snapshot.presentation.identity.clone();
    let source_play = snapshot.presentation.basis.active_play_id.clone();
    let server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || server.serve_count(1));

    let _silent_client = TcpStream::connect(address).unwrap();
    let failure = worker.join().unwrap().unwrap_err();

    assert_eq!(
        failure.snapshot.renderer.manifestation.lifecycle,
        ManifestationLifecycle::Failed
    );
    assert_eq!(
        failure.snapshot.renderer.manifestation.failure,
        Some(ManifestationFailure::DeliveryFailed)
    );
    assert_eq!(failure.snapshot.presentation.identity, source_identity);
    assert_eq!(
        failure.snapshot.presentation.basis.active_play_id,
        source_play
    );
    let sign = failure
        .snapshot
        .renderer
        .manifestation
        .signs
        .last()
        .unwrap();
    assert_eq!(
        sign.manifestation_id,
        failure.snapshot.renderer.manifestation.manifestation_id
    );
    assert_eq!(
        sign.plan_id,
        failure.snapshot.renderer.manifestation.plan_id
    );
    assert_eq!(
        sign.active_play_id,
        failure.snapshot.renderer.manifestation.active_play_id
    );
}
