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

#[test]
fn exact_read_only_routes_are_bounded_no_store_and_typed() {
    let index = request("/", "GET");
    assert!(index.starts_with("HTTP/1.1 200 OK"));
    assert!(index.contains("Cache-Control: no-store"));
    assert!(index.contains("X-Content-Type-Options: nosniff"));
    assert!(index.contains("Content-Security-Policy: default-src 'self'"));
    assert!(index.contains("One truth, rendered for the browser"));

    let response = request("/api/snapshot", "GET");
    let body = response.split("\r\n\r\n").nth(1).unwrap();
    let decoded = patchbay_html::RendererSnapshot::decode(body.as_bytes(), 1).unwrap();
    assert_eq!(decoded.revision, 1);
    assert_eq!(
        decoded.renderer.manifestation.lifecycle,
        ManifestationLifecycle::Available
    );
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
