use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::thread;

use conduit_core::{
    AuthorityTime, Id, PlanValidationContext, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION,
    SchedulerPolicy,
};
use conduit_runtime::{ExactRunContext, RunIo, SchedulerReservation};

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
    let observed_authority = installed
        .input
        .candidates
        .iter()
        .flat_map(|candidate| &candidate.authorities)
        .find(|authority| authority.grant.id == conduit_http::HTTP_CLIENT_LOOPBACK_GRANT)
        .unwrap();
    assert_eq!(observed_authority.effect.constraints.len(), 3);
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
    assert_eq!(authority.effect.constraints.len(), 3);
    for (id, value) in [
        ("conduit.constraint/http-authority", "loopback.test"),
        ("conduit.constraint/http-endpoint", "127.0.0.1:38153"),
        ("conduit.constraint/http-transport", "http"),
    ] {
        assert!(authority.effect.constraints.iter().any(|constraint| {
            constraint.id.as_str() == id
                && constraint.semantic_hash
                    == conduit_runtime::hosted_effect_constraint_hash(id, value.as_bytes())
        }));
    }
    assert!(authority.commit_profile.is_some());
    let profile = client.execution_profile.unwrap();
    assert!(profile.limits.max_pending_operations > 0);
    assert!(profile.limits.max_input_bytes > 0);
    assert!(profile.limits.max_output_bytes > 0);
}

#[test]
fn authored_network_authority_and_dns_facts_are_rejected() {
    let mut registry = conduit_runtime::Registry::hosted_primitives();
    conduit_http::register_hosted_http_client_provider(&mut registry).unwrap();
    let base = fs::read_to_string(root().join("examples/http-client-loopback.panel")).unwrap();
    for key in [
        "network_resource",
        "outbound_grant",
        "dns_observation",
        "provider_observation",
        "tls_policy",
        "trust_handle",
        "resource_lease",
        "host_observation",
        "destination_allowed",
    ] {
        let source = base.replace(
            "    address =",
            &format!("    {key} = secret(\"forged/fresh\")\n    address ="),
        );
        let error = conduit_compile::InstalledProfile::observe_registry(&source, &registry)
            .err()
            .expect("source-authored authority is outside the current grammar");
        assert_eq!(error.code, "CND-SRC-002", "{key}");
    }
}

#[test]
fn hostname_resolution_is_not_an_http_handler_effect() {
    let source = fs::read_to_string(root().join("examples/http-client-loopback.panel"))
        .unwrap()
        .replace("127.0.0.1:38153", "localhost:38153");
    let mut registry = conduit_runtime::Registry::hosted_primitives();
    conduit_http::register_hosted_http_client_provider(&mut registry).unwrap();
    let error = conduit_compile::InstalledProfile::observe_registry(&source, &registry)
        .err()
        .expect("the checked provider accepts only already-observed numeric endpoints");
    assert_eq!(error.code, "CND-HTTP-CL-004");
}

#[test]
fn compatibility_handler_cannot_bypass_the_exact_effect_backend() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let source = fs::read_to_string(root().join("examples/http-client-loopback.panel"))
        .unwrap()
        .replace(
            "127.0.0.1:38153",
            &listener.local_addr().unwrap().to_string(),
        );
    let mut registry = conduit_runtime::Registry::hosted_primitives();
    conduit_http::register_hosted_http_client_provider(&mut registry).unwrap();
    let panel = conduit_panel::parse(&source).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error_output = Vec::new();
    let mut display = Vec::new();
    let error = resolved
        .run_batch(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error_output,
            display: &mut display,
        })
        .unwrap_err();
    assert_eq!(error.code, "CND-HTTP-CL-020");
    assert!(matches!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    ));
}

#[test]
fn dns_and_doh_execution_remain_blocked_by_http_provider_registration() {
    let mut registry = conduit_runtime::Registry::hosted_primitives();
    conduit_http::register_hosted_http_client_provider(&mut registry).unwrap();
    for (contract, expected) in [
        ("net/dns/resolve", "CND-IMP-001"),
        ("net/dns/doh", "CND-IMP-001"),
    ] {
        let panel = conduit_panel::parse(&format!("panel 0\nresolver: {contract}\n")).unwrap();
        let error = registry
            .resolve(&panel)
            .expect_err("HTTP installation cannot create a DNS execution path");
        assert_eq!(error.code, expected, "{contract}");
    }
}

#[test]
fn revoked_grant_stale_provider_and_lease_drift_stop_before_socket_use() {
    for case in ["revoked", "stale-provider", "lease-drift"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let source = fs::read_to_string(root().join("examples/http-client-loopback.panel"))
            .unwrap()
            .replace(
                "127.0.0.1:38153",
                &listener.local_addr().unwrap().to_string(),
            );
        let mut registry = conduit_runtime::Registry::hosted_primitives();
        conduit_http::register_hosted_http_client_provider(&mut registry).unwrap();
        let mut installed =
            conduit_compile::InstalledProfile::observe_registry(&source, &registry).unwrap();
        let document = conduit_compile::compile_source(&source, &installed.input).unwrap();
        if case != "stale-provider" {
            let authority = installed
                .input
                .candidates
                .iter_mut()
                .flat_map(|candidate| &mut candidate.authorities)
                .find(|authority| authority.grant.id == conduit_http::HTTP_CLIENT_LOOPBACK_GRANT)
                .unwrap();
            if case == "revoked" {
                authority.status = "revoked".to_owned();
            } else {
                authority.resource_lease.resource_binding = "forged/http-binding".to_owned();
            }
        }
        let arena = bumpalo::Bump::new();
        let plan = document.as_plan(&arena).unwrap();
        let panel = conduit_panel::parse(&source).unwrap();
        let resolved = registry.resolve(&panel).unwrap();
        let bindings = installed.bindings(&plan).unwrap();
        let observations = installed.grant_observations(&plan).unwrap();
        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error_output = Vec::new();
        let mut display = Vec::new();
        let now = if case == "stale-provider" {
            AuthorityTime {
                basis: plan.created_at.basis,
                tick: 20,
            }
        } else {
            plan.created_at
        };
        let error = resolved
            .run_exact_report(
                &plan,
                &bindings,
                ExactRunContext {
                    semantic_source_hash: plan.source_semantic_hash,
                    plan_epoch: 1,
                    run_id: Id("fixture/http-authority-denied"),
                    validation: PlanValidationContext {
                        supported_schema_version: plan.schema_version,
                        now,
                    },
                    scheduler_policy: SchedulerPolicy {
                        schema_version: SCHEDULER_CONTRACT_VERSION,
                        ready_queue: ReadyQueueDiscipline::RoundRobin,
                        max_decisions: 256,
                        max_tick: 512,
                        max_consecutive_yields: 8,
                        max_events: 128,
                    },
                    reservation: SchedulerReservation {
                        available_runtime_memory_bytes: plan.budget.memory_bytes,
                        executor_overhead_limit_bytes: plan.budget.memory_bytes,
                    },
                    grant_observations: &observations,
                },
                &mut RunIo {
                    input: &mut input,
                    output: &mut output,
                    error: &mut error_output,
                    display: &mut display,
                },
            )
            .unwrap_err();
        if case == "stale-provider" {
            assert_eq!(error.code, "CND-HST-002");
        } else {
            assert_eq!(error.code, "CND-RUN-010");
        }
        assert!(matches!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        ));
    }
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
