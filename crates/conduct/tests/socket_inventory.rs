use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

fn invoke(source: &str, stdin: &[u8]) -> std::process::Output {
    let path = std::env::temp_dir().join(format!(
        "conduit-socket-panel-{}-{}.panel",
        std::process::id(),
        source.len()
    ));
    fs::write(&path, source).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .arg("--enable-socket-loopback")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let output = child.wait_with_output().unwrap();
    let _ = fs::remove_file(path);
    output
}

#[test]
fn socket_exact_plans_pin_provider_limits_and_authority() {
    let root = root();
    for (file, contract_id, implementation, grant, action) in [
        (
            "socket-tcp-connect.panel",
            "conduit.host/net/tcp/connect",
            "conduit/socket-linux-tcp-connect",
            "conduit.grant/socket-tcp-connect",
            "conduit.action/connect",
        ),
        (
            "socket-tcp-listen.panel",
            "conduit.host/net/tcp/listen",
            "conduit/socket-linux-tcp-listen",
            "conduit.grant/socket-tcp-listen",
            "conduit.action/listen",
        ),
        (
            "socket-udp-connected.panel",
            "conduit.host/net/udp/connected",
            "conduit/socket-linux-udp-connected",
            "conduit.grant/socket-udp-connected",
            "conduit.action/connect",
        ),
        (
            "socket-udp-datagram.panel",
            "conduit.host/net/udp/datagram",
            "conduit/socket-linux-udp-datagram",
            "conduit.grant/socket-udp-datagram",
            "conduit.action/bind",
        ),
    ] {
        let source = fs::read_to_string(root.join("examples").join(file)).unwrap();
        let absent = conduit_compile::InstalledProfile::observe_registry(
            &source,
            &conduit_runtime::Registry::hosted_primitives(),
        )
        .err()
        .expect("socket providers are opt-in");
        assert_eq!(absent.code, "CND-IMP-001");

        let mut registry = conduit_runtime::Registry::hosted_primitives();
        conduit_socket::register_hosted_socket_providers(&mut registry).unwrap();
        let installed =
            conduit_compile::InstalledProfile::observe_registry(&source, &registry).unwrap();
        let document = conduit_compile::compile_source(&source, &installed.input).unwrap();
        let arena = bumpalo::Bump::new();
        let plan = document.as_plan(&arena).unwrap();
        let node = plan
            .nodes
            .iter()
            .find(|node| node.contract.id.as_str() == contract_id)
            .unwrap();
        assert_eq!(node.implementation.id.as_str(), implementation);
        let profile = node.execution_profile.unwrap();
        assert_eq!(
            profile.limits.max_input_bytes,
            conduit_std::SOCKET_MAX_STREAM_BYTES as u64
        );
        assert_eq!(profile.limits.max_pending_operations, 4);
        assert_eq!(profile.limits.max_timers, 2);
        let authority = plan
            .authorities
            .iter()
            .find(|authority| authority.node == node.instance)
            .unwrap();
        assert_eq!(authority.grant.id.as_str(), grant);
        assert_eq!(authority.effect.action.as_str(), action);
        assert_eq!(
            authority.binding.resource.id.as_str(),
            conduit_socket::LOOPBACK_NETWORK_RESOURCE
        );
        assert!(authority.commit_profile.is_some());
    }
}

#[test]
fn hosted_loopback_runs_tcp_and_udp_and_cleans_up_cancellation() {
    let root = root();
    for file in ["socket-tcp-connect.panel", "socket-tcp-listen.panel"] {
        let source = fs::read_to_string(root.join("examples").join(file)).unwrap();
        let output = invoke(&source, b"bounded loopback\n");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"bounded loopback\n");
    }
    for file in ["socket-udp-connected.panel", "socket-udp-datagram.panel"] {
        let source = fs::read_to_string(root.join("examples").join(file)).unwrap();
        let output = invoke(&source, b"");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let source = fs::read_to_string(root.join("examples/socket-tcp-connect.panel")).unwrap();
    let cancelled = source.replace(
        r#"cancellation = "none""#,
        r#"cancellation = "cancel-after-commit""#,
    );
    let output = invoke(&cancelled, b"cancel");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("CND-SOCK-020"));

    let denied = source.replace(
        conduit_socket::TCP_CONNECT_GRANT,
        "conduit.grant/socket-denied",
    );
    let output = invoke(&denied, b"denied");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("CND-SOCK-010"));
}
