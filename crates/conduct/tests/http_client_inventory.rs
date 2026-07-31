use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::thread;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

#[test]
fn http_client_exact_plan_pins_provider_resource_grant_and_limits() {
    let source = fs::read_to_string(root().join("examples/http-client-loopback.panel")).unwrap();
    let absent = conduit_compile::InstalledProfile::observe_registry(
        &source,
        &conduit_runtime::Registry::hosted_primitives(),
    )
    .err()
    .expect("HTTP client provider is opt-in");
    assert_eq!(absent.code, "CND-IMP-001");

    let mut registry = conduit_runtime::Registry::hosted_primitives();
    conduit_http::register_hosted_http_client_provider(&mut registry).unwrap();
    let installed =
        conduit_compile::InstalledProfile::observe_registry(&source, &registry).unwrap();
    let document = conduit_compile::compile_source(&source, &installed.input).unwrap();
    let arena = bumpalo::Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let client = plan
        .nodes
        .iter()
        .find(|node| node.contract.id.as_str() == "net/http/fetch")
        .unwrap();
    assert_eq!(
        client.implementation.id.as_str(),
        "conduit/http-linux-client"
    );
    assert_eq!(
        client.artifact.as_str(),
        "conduit/http-linux-client-artifact"
    );
    assert_eq!(client.host.as_str(), "conduit/conduct-host");
    assert!(
        client
            .required_resources
            .iter()
            .any(|resource| resource.as_str() == conduit_http::HTTP_CLIENT_LOOPBACK_RESOURCE)
    );
    let authority = plan
        .authorities
        .iter()
        .find(|authority| authority.node == client.instance)
        .unwrap();
    assert_eq!(
        authority.grant.id.as_str(),
        conduit_http::HTTP_CLIENT_LOOPBACK_GRANT
    );
    assert_eq!(
        authority.binding.resource.id.as_str(),
        conduit_http::HTTP_CLIENT_LOOPBACK_RESOURCE
    );
    assert_eq!(authority.effect.action.as_str(), "conduit.action/request");
    assert!(authority.commit_profile.is_some());
    let profile = client.execution_profile.unwrap();
    assert!(profile.limits.max_pending_operations > 0);
    assert!(profile.limits.max_input_bytes > 0);
    assert!(profile.limits.max_output_bytes > 0);
}

#[test]
fn hosted_http_client_rejects_an_unsupported_redirect_profile_before_execution() {
    let source = fs::read_to_string(root().join("examples/http-client-loopback.panel"))
        .unwrap()
        .replace(
            "redirect_policy = \"return\"",
            "redirect_policy = \"same-authority\"",
        );
    let mut registry = conduit_runtime::Registry::hosted_primitives();
    conduit_http::register_hosted_http_client_provider(&mut registry).unwrap();
    let error = conduit_compile::InstalledProfile::observe_registry(&source, &registry)
        .err()
        .expect("unsupported hosted redirect policy fails during resolution");
    assert_eq!(error.code, "CND-HTTP-CL-017");
}

#[test]
fn hosted_http_client_runs_one_real_loopback_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let read = stream.read(&mut request).unwrap();
        assert!(request[..read].starts_with(b"GET /health HTTP/1.1\r\nHost: loopback.test\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nready")
            .unwrap();
    });

    let source = fs::read_to_string(root().join("examples/http-client-loopback.panel"))
        .unwrap()
        .replace("127.0.0.1:38153", &address.to_string());
    let path = std::env::temp_dir().join(format!(
        "conduit-http-client-panel-{}.panel",
        std::process::id()
    ));
    fs::write(&path, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .arg("--enable-http-client-loopback")
        .arg(&path)
        .output()
        .unwrap();
    let _ = fs::remove_file(path);
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
}
