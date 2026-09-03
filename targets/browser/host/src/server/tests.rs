use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_path(label: &str) -> std::path::PathBuf {
    let name = format!(
        "conduit-browser-host-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    std::env::temp_dir().join(name)
}

fn runtime_fixture() -> std::path::PathBuf {
    let path = unique_path("runtime.wasm");
    std::fs::write(&path, b"bounded wasm fixture").unwrap();
    path
}

fn application_fixture() -> std::path::PathBuf {
    let path = unique_path("application");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(
        path.join("index.html"),
        b"<!doctype html><title>fixture</title>",
    )
    .unwrap();
    std::fs::write(path.join("application.mjs"), b"export const exact = true;").unwrap();
    path
}

#[test]
fn simultaneous_host_and_application_entrances_are_distinct_loopback_listeners() {
    let runtime = runtime_fixture();
    let application = application_fixture();
    let first = BrowserHostServer::bind(&runtime).unwrap();
    let second = BrowserHostServer::bind_application(&application, "/fixture/").unwrap();
    let first_address = first.local_addr().unwrap();
    let second_address = second.local_addr().unwrap();
    assert_eq!(first_address.ip(), Ipv4Addr::LOCALHOST);
    assert_eq!(second_address.ip(), Ipv4Addr::LOCALHOST);
    assert_ne!(first_address, second_address);
    assert_eq!(
        second.url().unwrap(),
        format!("http://{second_address}/fixture/")
    );
    std::fs::remove_file(runtime).unwrap();
    std::fs::remove_dir_all(application).unwrap();
}

#[test]
fn missing_and_oversized_runtime_artifacts_refuse_before_launch() {
    let missing = unique_path("absent.wasm");
    assert!(BrowserHostServer::bind(&missing)
        .unwrap_err()
        .contains("unavailable"));

    let runtime = runtime_fixture();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&runtime)
        .unwrap();
    file.set_len(MAX_RUNTIME_BYTES as u64 + 1).unwrap();
    assert!(BrowserHostServer::bind(&runtime)
        .unwrap_err()
        .contains("artifact bound"));
    std::fs::remove_file(runtime).unwrap();
}

#[test]
fn generic_application_delivery_is_mount_scoped_and_bounded() {
    let directory = application_fixture();
    let application =
        application::ApplicationDirectory::admit(&directory, "/products/one/").unwrap();
    let index = application
        .response(Some("GET /products/one/ HTTP/1.1"))
        .unwrap()
        .unwrap();
    assert_eq!(index.content_type, "text/html; charset=utf-8");
    let module = application
        .response(Some("GET /products/one/application.mjs HTTP/1.1"))
        .unwrap()
        .unwrap();
    assert_eq!(module.body, b"export const exact = true;");
    assert!(application
        .response(Some("GET /products/two/application.mjs HTTP/1.1"))
        .unwrap()
        .is_none());
    assert!(application
        .response(Some("GET /products/one/../application.mjs HTTP/1.1"))
        .unwrap()
        .is_none());
    assert!(application::ApplicationDirectory::admit(&directory, "/bad/../mount/").is_err());

    let oversized = directory.join("oversized.bin");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(application::MAX_APPLICATION_FILE_BYTES + 1)
        .unwrap();
    assert!(application
        .response(Some("GET /products/one/oversized.bin HTTP/1.1"))
        .unwrap_err()
        .contains("delivery bound"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn bare_host_serves_the_bounded_generic_operation_adapter() {
    let runtime = runtime_fixture();
    let server = BrowserHostServer::bind(&runtime).unwrap();
    let address = server.local_addr().unwrap();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let (mut stream, _) = server.listener.accept().unwrap();
            server.respond(&mut stream).unwrap();
        });
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(
                b"GET /assets/browser-host-operations.mjs HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        let response = String::from_utf8(response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: text/javascript; charset=utf-8"));
        assert!(response.contains("conduit.host/browser-effects@1"));
        assert!(response.contains("createBrowserHostOperations"));
    });
    std::fs::remove_file(runtime).unwrap();
}
