use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn runtime_fixture() -> std::path::PathBuf {
    let name = format!(
        "conduit-browser-host-runtime-{}-{}.wasm",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, b"bounded wasm fixture").unwrap();
    path
}

#[test]
fn simultaneous_entrances_are_distinct_ipv4_loopback_listeners() {
    let runtime = runtime_fixture();
    let first = BrowserHostServer::bind(&runtime, ProductSurface::Book).unwrap();
    let second = BrowserHostServer::bind(&runtime, ProductSurface::Creche).unwrap();
    let first_address = first.local_addr().unwrap();
    let second_address = second.local_addr().unwrap();
    assert_eq!(first_address.ip(), Ipv4Addr::LOCALHOST);
    assert_eq!(second_address.ip(), Ipv4Addr::LOCALHOST);
    assert_ne!(first_address, second_address);
    std::fs::remove_file(runtime).unwrap();
}

#[test]
fn missing_and_oversized_runtime_artifacts_refuse_before_launch() {
    let missing = std::env::temp_dir().join("conduit-browser-host-absent.wasm");
    assert!(BrowserHostServer::bind(&missing, ProductSurface::Host)
        .unwrap_err()
        .contains("unavailable"));

    let runtime = runtime_fixture();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&runtime)
        .unwrap();
    file.set_len(MAX_RUNTIME_BYTES as u64 + 1).unwrap();
    assert!(BrowserHostServer::bind(&runtime, ProductSurface::Host)
        .unwrap_err()
        .contains("artifact bound"));
    std::fs::remove_file(runtime).unwrap();
}

#[test]
fn product_surfaces_refuse_the_other_products_routes() {
    assert!(ProductSurface::Book.permits(Some("GET /book/ HTTP/1.1")));
    assert!(ProductSurface::Book.permits(Some("GET /book/runtime.wasm HTTP/1.1")));
    assert!(!ProductSurface::Book.permits(Some("GET /creche/ HTTP/1.1")));
    assert!(!ProductSurface::Book.permits(Some("GET / HTTP/1.1")));

    assert!(ProductSurface::Creche.permits(Some("GET /creche/ HTTP/1.1")));
    assert!(ProductSurface::Creche.permits(Some("GET /creche/runtime.wasm HTTP/1.1")));
    assert!(!ProductSurface::Creche.permits(Some("GET /book/ HTTP/1.1")));
    assert!(!ProductSurface::Creche.permits(Some("GET / HTTP/1.1")));
}
