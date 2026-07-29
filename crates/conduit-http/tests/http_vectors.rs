use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpStream};
use std::sync::Arc;
use std::task::Poll;
use std::thread;
use std::time::{Duration, Instant};

use conduit_core::{
    ArtifactDigest, Id, PinnedDescriptor, PlanArtifact, PlanResourceBudget, SemanticHash,
};
use conduit_http::{
    DirectTlsSecretHandles, HTTP_NODE_CONTRACTS, HTTP_TYPE_CONTRACTS, HostedHttpEvidence,
    HttpAsset, HttpEvidenceKind, HttpExchangeEvent, HttpExchangeId, HttpHeader, HttpMethod,
    HttpNodeKind, HttpProtocol, HttpReason, HttpResponsePart, HttpRoute, HttpSecurityMode,
    HttpServiceLimits, HttpServingAuthority, HttpServingBackend, HttpServingCapabilities,
    HttpTransition, InMemoryHttpServingBackend, LinuxHttpServingBackend, ResolvedHttpSelection,
    ResolvedHttpService, SERVE_COMPOSITE_BOUNDARY, SecretFileHandle, SessionKind, TrustedProxy,
    ViewProjectionBinding, match_route, resolve_asset, validate_certificate_window,
    validate_http_selection, validate_http_transition, validate_view_projection,
};
use conduit_runtime::ResolvedPlacementBinding;
use rcgen::generate_simple_self_signed;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use serde::Deserialize;
use serde_json::{Value, json};
use tempfile::tempdir;

const FIXTURE: &str = include_str!("../../../conformance/c5/http-serving-v1.json");

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    runner: String,
    expected: Value,
}

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn descriptor(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

fn capabilities(mode: HttpSecurityMode) -> HttpServingCapabilities {
    HttpServingCapabilities {
        profile_version: 1,
        plaintext: mode == HttpSecurityMode::Plaintext,
        direct_tls: mode == HttpSecurityMode::DirectTls,
        trusted_proxy_tls: mode == HttpSecurityMode::TrustedProxyTls,
        http11: true,
        http2: false,
        websocket: true,
        sse: true,
        maximum_request_head_bytes: 1024,
        maximum_request_body_bytes: 32,
        maximum_response_bytes: 64,
        maximum_connections: 2,
        maximum_sessions: 1,
        adapter_buffer_bytes: 1024,
        backend_buffer_bytes: 1024,
        kernel_buffer_bytes: 1024,
        complete_stack_hard_bounded: true,
    }
}

fn limits() -> HttpServiceLimits {
    HttpServiceLimits {
        maximum_request_head_bytes: 256,
        maximum_request_body_bytes: 16,
        maximum_response_bytes: 32,
        maximum_header_count: 8,
        maximum_header_bytes: 128,
        maximum_connections: 2,
        maximum_queued_admissions: 1,
        maximum_live_handlers: 2,
        maximum_sessions: 1,
        maximum_session_queue_items: 4,
        maximum_session_queue_bytes: 64,
        maximum_evidence_events: 16,
        header_deadline_ticks: 10,
        body_deadline_ticks: 10,
        handler_deadline_ticks: 10,
        drain_deadline_ticks: 10,
        reserved_memory_bytes: 4096,
    }
}

fn unsealed_service(mode: HttpSecurityMode, listen: &'static str) -> ResolvedHttpService<'static> {
    ResolvedHttpService {
        identity: hash(0),
        service: descriptor("conduit.http/service", 1),
        backend: descriptor(
            if mode == HttpSecurityMode::DirectTls {
                "conduit/http.linux-rustls"
            } else {
                "conduit/http.in-memory"
            },
            2,
        ),
        artifact: PlanArtifact {
            id: Id("artifact/http"),
            digest: ArtifactDigest::from_bytes([3; 32]),
        },
        execution_profile: descriptor("profile/http", 4),
        listen,
        protocol: HttpProtocol::Http11,
        security: descriptor("security/http", 5),
        security_mode: mode,
        certificate_identity: (mode == HttpSecurityMode::DirectTls).then(|| hash(6)),
        trusted_proxy: (mode == HttpSecurityMode::TrustedProxyTls).then(|| TrustedProxy {
            identity: hash(7),
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            accepts_forwarded_scheme: true,
            accepts_forwarded_client: true,
            accepts_forwarded_principal: true,
        }),
        grant: Id("grant/serve"),
        secret_scope: (mode == HttpSecurityMode::DirectTls).then_some(Id("secret/tls")),
        require_complete_stack_hard_bound: false,
        limits: limits(),
    }
}

fn service(mode: HttpSecurityMode) -> ResolvedHttpService<'static> {
    service_at(mode, "127.0.0.1:0")
}

fn service_at(mode: HttpSecurityMode, listen: &'static str) -> ResolvedHttpService<'static> {
    let mut service = unsealed_service(mode, listen);
    service.identity = service.computed_identity();
    service
}

fn linux_service(mode: HttpSecurityMode) -> ResolvedHttpService<'static> {
    let mut service = unsealed_service(mode, "127.0.0.1:0");
    service.backend = descriptor("conduit/http.linux-rustls", 2);
    service.identity = service.computed_identity();
    service
}

fn authority() -> HttpServingAuthority<'static> {
    HttpServingAuthority {
        grant: Id("grant/serve"),
        allowed: true,
        current_tick: 10,
        valid_until_tick: 100,
    }
}

fn placement(service: &ResolvedHttpService<'_>) -> ResolvedPlacementBinding {
    ResolvedPlacementBinding {
        instance: "server".to_owned(),
        semantic_contract: service.service.semantic_hash,
        implementation_id: service.backend.id.as_str().to_owned(),
        implementation_identity: service.backend.semantic_hash,
        host: "host/linux".to_owned(),
        report_id: "report/linux".to_owned(),
        report_identity: hash(9),
        report_time_basis: "fixture".to_owned(),
        report_observed_at_tick: 1,
        report_valid_until_tick: 100,
        allocation: PlanResourceBudget {
            memory_bytes: service.limits.reserved_memory_bytes,
            storage_bytes: 0,
            cpu_units: 1,
            timers: 4,
            transports: 2,
            checkpoints: 0,
            evidence_bytes: 4096,
        },
        artifacts: vec![(
            service.artifact.id.as_str().to_owned(),
            service.artifact.digest,
        )],
        capability_subjects: vec!["http".to_owned()],
        capability_proofs: vec![hash(10)],
        resource_ids: vec!["tcp/listener".to_owned()],
        authority_grants: vec!["grant/serve".to_owned()],
    }
}

fn selection(
    service: &ResolvedHttpService<'static>,
    offered: HttpServingCapabilities,
) -> ResolvedHttpSelection<'static> {
    ResolvedHttpSelection {
        backend: service.backend,
        artifact: service.artifact,
        execution_profile: service.execution_profile,
        endpoint: service.listen,
        security: service.security,
        security_mode: service.security_mode,
        capabilities: offered,
    }
}

fn request(path: &str) -> Vec<u8> {
    format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n").into_bytes()
}

fn proxy_request() -> Vec<u8> {
    b"GET / HTTP/1.1\r\nHost: public.example\r\nX-Forwarded-Proto: https\r\nX-Forwarded-For: 192.0.2.8\r\nX-Authenticated-Principal: entity/alice\r\nContent-Length: 0\r\n\r\n".to_vec()
}

fn accepted_memory(mode: HttpSecurityMode) -> InMemoryHttpServingBackend {
    let mut backend = InMemoryHttpServingBackend::new(capabilities(mode));
    backend.bind(&service(mode), authority()).unwrap();
    backend
}

fn next_request(
    backend: &mut InMemoryHttpServingBackend,
    bytes: &[u8],
) -> (u64, conduit_http::HttpRequest) {
    let exchange = backend
        .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), bytes)
        .unwrap();
    let connection = match backend.poll_accept() {
        Poll::Ready(Ok(connection)) => connection,
        other => panic!("expected accepted connection, got {other:?}"),
    };
    let request = match backend.poll_exchange(connection) {
        Poll::Ready(Ok(HttpExchangeEvent::Request(request))) => request,
        other => panic!("expected request, got {other:?}"),
    };
    assert_eq!(request.exchange, exchange);
    (connection, request)
}

fn result(error: Option<HttpReason>) -> Value {
    match error {
        Some(error) => json!({"accepted": false, "code": error.code()}),
        None => json!({"accepted": true}),
    }
}

fn contract_case(id: &str) -> Value {
    match id {
        "domain-types-stay-outside-core" => {
            assert_eq!(HTTP_TYPE_CONTRACTS.len(), 13);
            assert!(
                HTTP_TYPE_CONTRACTS
                    .iter()
                    .all(|kind| kind.id().starts_with("conduit.http/"))
            );
            json!({"accepted": true})
        }
        "serve-is-an-exported-composite" => {
            assert!(HTTP_NODE_CONTRACTS.contains(&HttpNodeKind::ServeComposite));
            assert_eq!(SERVE_COMPOSITE_BOUNDARY.request_output.as_str(), "requests");
            assert_eq!(
                SERVE_COMPOSITE_BOUNDARY.response_input.as_str(),
                "responses"
            );
            json!({"accepted": true})
        }
        "nested-view-reuses-domain-exports" => {
            validate_view_projection(ViewProjectionBinding {
                domain_instance: Id("domain/existing"),
                domain_state_port: Id("state"),
                client_intent_port: Id("intent"),
                view_projector: Id("view/status"),
                maximum_view_update_bytes: 1024,
                maximum_pending_updates: 4,
            })
            .unwrap();
            json!({"accepted": true})
        }
        other => panic!("unknown contract case {other}"),
    }
}

fn selection_case(id: &str) -> Value {
    let service = service(HttpSecurityMode::Plaintext);
    let baseline_placement = placement(&service);
    match id {
        "exact-resolver-selection" => {
            validate_http_selection(
                &service,
                &baseline_placement,
                selection(&service, capabilities(HttpSecurityMode::Plaintext)),
            )
            .unwrap();
            json!({"accepted": true})
        }
        "artifact-drift-rejected" => {
            let mut selected = selection(&service, capabilities(HttpSecurityMode::Plaintext));
            selected.artifact.digest = ArtifactDigest::from_bytes([99; 32]);
            result(validate_http_selection(&service, &baseline_placement, selected).err())
        }
        "resource-underaccounted-rejected" => {
            let mut offered = capabilities(HttpSecurityMode::Plaintext);
            offered.maximum_request_body_bytes = 1;
            result(
                validate_http_selection(
                    &service,
                    &baseline_placement,
                    selection(&service, offered),
                )
                .err(),
            )
        }
        "serving-grant-missing-rejected" => {
            let mut missing_grant_placement = placement(&service);
            missing_grant_placement.authority_grants.clear();
            result(
                validate_http_selection(
                    &service,
                    &missing_grant_placement,
                    selection(&service, capabilities(HttpSecurityMode::Plaintext)),
                )
                .err(),
            )
        }
        "high-assurance-observed-stack-rejected" => {
            let mut service = service;
            service.require_complete_stack_hard_bound = true;
            service.identity = service.computed_identity();
            let mut offered = capabilities(HttpSecurityMode::Plaintext);
            offered.complete_stack_hard_bounded = false;
            result(
                validate_http_selection(
                    &service,
                    &placement(&service),
                    selection(&service, offered),
                )
                .err(),
            )
        }
        "constrained-host-unsupported-before-start" => {
            let mut offered = capabilities(HttpSecurityMode::Plaintext);
            offered.plaintext = false;
            result(
                validate_http_selection(
                    &service,
                    &baseline_placement,
                    selection(&service, offered),
                )
                .err(),
            )
        }
        other => panic!("unknown selection case {other}"),
    }
}

fn routing_case(id: &str) -> Value {
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
    match id {
        "route-order-is-explicit" => {
            json!({"route": match_route(&routes, HttpMethod::Get, "/users/42").unwrap().unwrap().route})
        }
        "route-fallthrough-is-deterministic" => {
            assert!(
                match_route(&routes, HttpMethod::Post, "/users/42")
                    .unwrap()
                    .is_none()
            );
            json!({"route": null})
        }
        "path-parameters-are-typed-output" => {
            let matched = match_route(&routes, HttpMethod::Get, "/users/42")
                .unwrap()
                .unwrap();
            json!({"id": matched.parameters["id"]})
        }
        other => panic!("unknown routing case {other}"),
    }
}

fn memory_case(id: &str) -> Value {
    match id {
        "response-correlation-mismatch-rejected" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let (connection, request) = next_request(&mut backend, &request("/"));
            let response = HttpResponsePart {
                exchange: HttpExchangeId {
                    connection: connection + 1,
                    request: request.exchange.request,
                },
                status: 200,
                headers: vec![],
                body: vec![],
                terminal: true,
            };
            let error = match backend.poll_send(connection, &response) {
                Poll::Ready(Err(error)) => error,
                other => panic!("expected correlation failure, got {other:?}"),
            };
            result(Some(error))
        }
        "bounded-request-response" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let (connection, request) = next_request(&mut backend, &request("/"));
            let response = HttpResponsePart {
                exchange: request.exchange,
                status: 200,
                headers: vec![],
                body: b"ok".to_vec(),
                terminal: true,
            };
            assert_eq!(
                backend.poll_send(connection, &response),
                Poll::Ready(Ok(()))
            );
            json!({"accepted": true, "status": 200})
        }
        "oversized-request-head-rejected" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let bytes = format!(
                "GET / HTTP/1.1\r\nX-Large: {}\r\nContent-Length: 0\r\n\r\n",
                "a".repeat(300)
            );
            result(
                backend
                    .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), bytes.as_bytes())
                    .err(),
            )
        }
        "header-count-rejected" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let mut bytes = "GET / HTTP/1.1\r\n".to_owned();
            for index in 0..9 {
                bytes.push_str(&format!("X-{index}: v\r\n"));
            }
            bytes.push_str("Content-Length: 0\r\n\r\n");
            result(
                backend
                    .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), bytes.as_bytes())
                    .err(),
            )
        }
        "oversized-body-rejected" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let body = "x".repeat(17);
            let bytes = format!("POST / HTTP/1.1\r\nContent-Length: 17\r\n\r\n{body}").into_bytes();
            result(backend.admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &bytes).err())
        }
        "malformed-request-rejected" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            result(
                backend
                    .admit(
                        IpAddr::V4(Ipv4Addr::LOCALHOST),
                        b"INVALID\r\nContent-Length: 0\r\n\r\n",
                    )
                    .err(),
            )
        }
        "admission-pressure-is-bounded" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            backend
                .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &request("/one"))
                .unwrap();
            result(
                backend
                    .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &request("/two"))
                    .err(),
            )
        }
        "slow-handler-times-out" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let exchange = backend
                .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &request("/"))
                .unwrap();
            backend.expire(exchange.connection).unwrap();
            let reason = backend
                .take_evidence()
                .into_iter()
                .chain(std::iter::from_fn(|| backend.take_evidence()))
                .find(|event| event.reason == Some(HttpReason::Timeout))
                .and_then(|event| event.reason);
            result(reason)
        }
        "slow-client-times-out" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let exchange = backend
                .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &request("/"))
                .unwrap();
            let _ = backend.poll_accept();
            backend.expire(exchange.connection).unwrap();
            let reason = std::iter::from_fn(|| backend.take_evidence())
                .find(|event| event.reason == Some(HttpReason::Timeout))
                .and_then(|event| event.reason);
            result(reason)
        }
        "cancellation-is-terminal" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let (connection, request) = next_request(&mut backend, &request("/"));
            backend.cancel(connection, request.exchange).unwrap();
            let error = match backend.poll_exchange(connection) {
                Poll::Ready(Err(error)) => error,
                other => panic!("expected cancellation, got {other:?}"),
            };
            result(Some(error))
        }
        "oversized-response-rejected" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let (connection, request) = next_request(&mut backend, &request("/"));
            let response = HttpResponsePart {
                exchange: request.exchange,
                status: 200,
                headers: vec![],
                body: vec![0; 33],
                terminal: true,
            };
            let error = match backend.poll_send(connection, &response) {
                Poll::Ready(Err(error)) => error,
                other => panic!("expected response rejection, got {other:?}"),
            };
            result(Some(error))
        }
        "oversized-response-headers-rejected" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let (connection, request) = next_request(&mut backend, &request("/"));
            let response = HttpResponsePart {
                exchange: request.exchange,
                status: 200,
                headers: (0..9)
                    .map(|index| HttpHeader {
                        name: format!("x-{index}"),
                        value: "value".to_owned(),
                    })
                    .collect(),
                body: vec![],
                terminal: true,
            };
            let error = match backend.poll_send(connection, &response) {
                Poll::Ready(Err(error)) => error,
                other => panic!("expected response-header rejection, got {other:?}"),
            };
            result(Some(error))
        }
        "evidence-capacity-is-bounded" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            for _ in 0..32 {
                let _ = backend.admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &request("/"));
            }
            let count = std::iter::from_fn(|| backend.take_evidence()).count();
            assert!(count <= 16);
            json!({"maximum": 16})
        }
        "evidence-exhaustion-precedes-admission" => {
            let mut exact_service = service(HttpSecurityMode::Plaintext);
            exact_service.limits.maximum_evidence_events = 2;
            exact_service.identity = exact_service.computed_identity();
            let mut backend =
                InMemoryHttpServingBackend::new(capabilities(HttpSecurityMode::Plaintext));
            backend.bind(&exact_service, authority()).unwrap();
            backend
                .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &request("/one"))
                .unwrap();
            assert!(matches!(backend.poll_accept(), Poll::Ready(Ok(_))));
            let before = backend.connection_count();
            let error = backend
                .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &request("/two"))
                .unwrap_err();
            assert_eq!(backend.connection_count(), before);
            result(Some(error))
        }
        other => panic!("unknown memory case {other}"),
    }
}

fn security_case(id: &str) -> Value {
    match id {
        "plaintext-must-be-explicit" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let (_, request) = next_request(&mut backend, &request("/"));
            assert!(!request.security.encrypted);
            json!({"accepted": true})
        }
        "required-tls-never-downgrades" => {
            let service = service(HttpSecurityMode::DirectTls);
            let mut offered = capabilities(HttpSecurityMode::Plaintext);
            offered.direct_tls = false;
            result(
                validate_http_selection(
                    &service,
                    &placement(&service),
                    selection(&service, offered),
                )
                .err(),
            )
        }
        "direct-tls-needs-secret-handles" => {
            let mut service = unsealed_service(HttpSecurityMode::DirectTls, "127.0.0.1:0");
            service.secret_scope = None;
            service.identity = service.computed_identity();
            result(service.validate().err())
        }
        "expired-serving-authority-rejected" => {
            let service = service(HttpSecurityMode::Plaintext);
            let mut backend =
                InMemoryHttpServingBackend::new(capabilities(HttpSecurityMode::Plaintext));
            result(
                backend
                    .bind(
                        &service,
                        HttpServingAuthority {
                            grant: Id("grant/serve"),
                            allowed: true,
                            current_tick: 100,
                            valid_until_tick: 100,
                        },
                    )
                    .err(),
            )
        }
        "trusted-proxy-security-accepted" => {
            let mut backend = accepted_memory(HttpSecurityMode::TrustedProxyTls);
            let (_, request) = next_request(&mut backend, &proxy_request());
            assert!(request.security.encrypted);
            assert_eq!(request.security.authenticated_proxy, Some(hash(7)));
            json!({"accepted": true, "encrypted": true})
        }
        "untrusted-forwarded-headers-rejected" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            result(
                backend
                    .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &proxy_request())
                    .err(),
            )
        }
        "wrong-proxy-peer-rejected" => {
            let mut backend = accepted_memory(HttpSecurityMode::TrustedProxyTls);
            result(
                backend
                    .admit(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), &proxy_request())
                    .err(),
            )
        }
        "proxy-principal-is-explicit" => {
            let mut backend = accepted_memory(HttpSecurityMode::TrustedProxyTls);
            let (_, request) = next_request(&mut backend, &proxy_request());
            json!({"principal": request.security.authenticated_principal.unwrap()})
        }
        "certificate-expiry-rejected" => result(validate_certificate_window(100, 10, 100).err()),
        other => panic!("unknown security case {other}"),
    }
}

fn session_case(id: &str) -> Value {
    match id {
        "websocket-session-is-bounded" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let (_, request) = next_request(&mut backend, &request("/ws"));
            backend
                .open_session(request.exchange, SessionKind::WebSocket)
                .unwrap();
            json!({"accepted": true})
        }
        "sse-session-is-bounded" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let (_, request) = next_request(&mut backend, &request("/events"));
            backend
                .open_session(request.exchange, SessionKind::ServerSentEvents)
                .unwrap();
            json!({"accepted": true})
        }
        "unsupported-upgrade-rejected" => {
            let mut offered = capabilities(HttpSecurityMode::Plaintext);
            offered.websocket = false;
            let mut backend = InMemoryHttpServingBackend::new(offered);
            backend
                .bind(&service(HttpSecurityMode::Plaintext), authority())
                .unwrap();
            let (_, request) = next_request(&mut backend, &request("/ws"));
            result(
                backend
                    .open_session(request.exchange, SessionKind::WebSocket)
                    .err(),
            )
        }
        "session-capacity-rejected" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let (first, request) = next_request(&mut backend, &request("/ws"));
            let response = HttpResponsePart {
                exchange: request.exchange,
                status: 101,
                headers: vec![],
                body: vec![],
                terminal: false,
            };
            assert_eq!(backend.poll_send(first, &response), Poll::Ready(Ok(())));
            backend
                .open_session(request.exchange, SessionKind::WebSocket)
                .unwrap();
            let error = backend
                .open_session(
                    HttpExchangeId {
                        connection: 99,
                        request: 1,
                    },
                    SessionKind::WebSocket,
                )
                .unwrap_err();
            result(Some(error))
        }
        "session-update-queue-is-bounded" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            let (_, request) = next_request(&mut backend, &request("/events"));
            backend
                .open_session(request.exchange, SessionKind::ServerSentEvents)
                .unwrap();
            assert_eq!(
                backend.session_kind(request.exchange),
                Some(SessionKind::ServerSentEvents)
            );
            for _ in 0..4 {
                backend
                    .send_session_update(request.exchange, b"0123456789")
                    .unwrap();
            }
            let error = backend
                .send_session_update(request.exchange, b"overflow")
                .unwrap_err();
            let first = backend
                .take_session_update(request.exchange)
                .unwrap()
                .unwrap();
            assert_eq!(first, b"0123456789");
            result(Some(error))
        }
        other => panic!("unknown session case {other}"),
    }
}

fn asset_case(id: &str) -> Value {
    let assets = [HttpAsset {
        path: "/app.js",
        media_type: "text/javascript",
        artifact_identity: hash(40),
        required_grant: Id("grant/assets"),
        bytes: b"console.log('bounded');",
    }];
    match id {
        "static-asset-exact-binding" => {
            let asset =
                resolve_asset(&assets, "/app.js", hash(40), Id("grant/assets"), 64).unwrap();
            assert_eq!(asset.unwrap().media_type, "text/javascript");
            json!({"accepted": true})
        }
        "static-asset-grant-mismatch" => {
            result(resolve_asset(&assets, "/app.js", hash(40), Id("grant/other"), 64).err())
        }
        "static-asset-size-rejected" => {
            result(resolve_asset(&assets, "/app.js", hash(40), Id("grant/assets"), 1).err())
        }
        other => panic!("unknown asset case {other}"),
    }
}

fn lifecycle_case(id: &str) -> Value {
    match id {
        "graceful-shutdown-stops-admission" => {
            let mut backend = accepted_memory(HttpSecurityMode::Plaintext);
            backend
                .admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &request("/"))
                .unwrap();
            let cancelled = backend.shutdown().unwrap();
            assert_eq!(
                backend.admit(IpAddr::V4(Ipv4Addr::LOCALHOST), &request("/")),
                Err(HttpReason::Closed)
            );
            json!({"cancelled": cancelled})
        }
        other => panic!("unknown lifecycle case {other}"),
    }
}

fn transition_case(id: &str) -> Value {
    let old = service_at(HttpSecurityMode::Plaintext, "127.0.0.1:8080");
    let candidate = service_at(HttpSecurityMode::Plaintext, "127.0.0.1:8081");
    match id {
        "transition-drains-and-rebinds" => {
            assert_eq!(
                validate_http_transition(&old, &candidate, 8192).unwrap(),
                HttpTransition::DrainAndRebind
            );
            json!({"transition": "drain-and-rebind"})
        }
        "transition-security-downgrade-rejected" => {
            let tls = service_at(HttpSecurityMode::DirectTls, "127.0.0.1:8443");
            result(validate_http_transition(&tls, &candidate, 8192).err())
        }
        "transition-overlap-must-fit" => {
            result(validate_http_transition(&old, &candidate, 4096).err())
        }
        other => panic!("unknown transition case {other}"),
    }
}

fn drive_plaintext_loopback() -> Value {
    let service = linux_service(HttpSecurityMode::Plaintext);
    let mut offered = capabilities(HttpSecurityMode::Plaintext);
    offered.complete_stack_hard_bounded = false;
    let mut backend = LinuxHttpServingBackend::new(offered, None).unwrap();
    backend.bind(&service, authority()).unwrap();
    let address = backend.local_addr().unwrap();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(&request("/health")).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    });
    let connection = poll_until(|| backend.poll_accept()).unwrap();
    let request = match poll_until(|| backend.poll_exchange(connection)).unwrap() {
        HttpExchangeEvent::Request(request) => request,
        other => panic!("unexpected event {other:?}"),
    };
    let response = HttpResponsePart {
        exchange: request.exchange,
        status: 200,
        headers: vec![],
        body: b"ok".to_vec(),
        terminal: true,
    };
    poll_until(|| backend.poll_send(connection, &response)).unwrap();
    backend.close(connection).unwrap();
    let bytes = client.join().unwrap();
    assert!(bytes.starts_with(b"HTTP/1.1 200 OK"));
    json!({"accepted": true, "encrypted": false})
}

fn drive_tls_loopback() -> Value {
    let directory = tempdir().unwrap();
    let certified = generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
    let certificate_path = directory.path().join("certificate.pem");
    let key_path = directory.path().join("key.pem");
    std::fs::write(&certificate_path, certified.cert.pem()).unwrap();
    std::fs::write(&key_path, certified.signing_key.serialize_pem()).unwrap();

    let service = linux_service(HttpSecurityMode::DirectTls);
    let mut offered = capabilities(HttpSecurityMode::DirectTls);
    offered.complete_stack_hard_bounded = false;
    let mut backend = LinuxHttpServingBackend::new(
        offered,
        Some(DirectTlsSecretHandles {
            certificate_chain: SecretFileHandle::new(certificate_path),
            private_key: SecretFileHandle::new(key_path),
        }),
    )
    .unwrap();
    backend.bind(&service, authority()).unwrap();
    let address = backend.local_addr().unwrap();

    let certificate = certified.cert.der().clone();
    let client = thread::spawn(move || {
        let mut roots = RootCertStore::empty();
        roots.add(certificate).unwrap();
        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let connection =
            ClientConnection::new(Arc::new(config), ServerName::try_from("localhost").unwrap())
                .unwrap();
        let socket = TcpStream::connect(address).unwrap();
        let mut stream = StreamOwned::new(connection, socket);
        stream.write_all(&request("/secure")).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    });

    let connection = poll_until(|| backend.poll_accept()).unwrap();
    let request = match poll_until(|| backend.poll_exchange(connection)).unwrap() {
        HttpExchangeEvent::Request(request) => request,
        other => panic!("unexpected event {other:?}"),
    };
    assert!(request.security.encrypted);
    let response = HttpResponsePart {
        exchange: request.exchange,
        status: 200,
        headers: vec![HttpHeader {
            name: "content-type".to_owned(),
            value: "text/plain".to_owned(),
        }],
        body: b"secure".to_vec(),
        terminal: true,
    };
    poll_until(|| backend.poll_send(connection, &response)).unwrap();
    backend.close(connection).unwrap();
    let bytes = client.join().unwrap();
    assert!(bytes.starts_with(b"HTTP/1.1 200 OK"));
    json!({"accepted": true, "encrypted": true})
}

fn poll_until<T>(
    mut operation: impl FnMut() -> Poll<Result<T, HttpReason>>,
) -> Result<T, HttpReason> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match operation() {
            Poll::Ready(result) => return result,
            Poll::Pending if Instant::now() < deadline => thread::yield_now(),
            Poll::Pending => return Err(HttpReason::Timeout),
        }
    }
}

fn linux_case(id: &str) -> Value {
    match id {
        "real-linux-plaintext-loopback" => drive_plaintext_loopback(),
        "real-linux-tls-loopback" => drive_tls_loopback(),
        other => panic!("unknown Linux case {other}"),
    }
}

fn execute(case: &Case) -> Value {
    match case.runner.as_str() {
        "http-contract" => contract_case(&case.id),
        "http-selection" => selection_case(&case.id),
        "http-routing" => routing_case(&case.id),
        "http-memory" => memory_case(&case.id),
        "http-security" => security_case(&case.id),
        "http-session" => session_case(&case.id),
        "http-asset" => asset_case(&case.id),
        "http-lifecycle" => lifecycle_case(&case.id),
        "http-transition" => transition_case(&case.id),
        "http-linux" => linux_case(&case.id),
        other => panic!("unknown runner {other}"),
    }
}

#[test]
fn every_http_fixture_case_executes_independently() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture.cases.len(), 49);
    for case in &fixture.cases {
        assert_eq!(execute(case), case.expected, "fixture {}", case.id);
    }
}

#[test]
fn evidence_distinguishes_transport_security_from_conduit_authority() {
    let mut backend = accepted_memory(HttpSecurityMode::TrustedProxyTls);
    let _ = next_request(&mut backend, &proxy_request());
    let evidence =
        std::iter::from_fn(|| backend.take_evidence()).collect::<Vec<HostedHttpEvidence>>();
    assert!(evidence.iter().any(|event| {
        event.kind == HttpEvidenceKind::RequestReceived
            && event.encrypted
            && event.proxy_authenticated
            && event.conduit_authority_checked
    }));
    assert!(
        evidence
            .iter()
            .all(|event| event.service_identity
                == service(HttpSecurityMode::TrustedProxyTls).identity)
    );
}
