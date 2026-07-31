use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Inventory {
    schema: String,
    states: Vec<String>,
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    path: String,
    state: String,
    profile: String,
    proof: String,
    expected_stdout: Option<String>,
    expected_stderr: Option<String>,
    expected_diagnostic: Option<String>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_panels(root: &Path, relative: &Path, found: &mut BTreeSet<String>) {
    let directory = root.join(relative);
    for entry in fs::read_dir(&directory).expect("panel inventory directory exists") {
        let entry = entry.expect("panel inventory entry is readable");
        let path = entry.path();
        if path.is_dir() {
            collect_panels(
                root,
                path.strip_prefix(root).expect("path remains under root"),
                found,
            );
        } else if path
            .extension()
            .is_some_and(|extension| extension == "panel")
        {
            found.insert(
                path.strip_prefix(root)
                    .expect("path remains under root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

fn invoke(root: &Path, entry: &Entry) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_conduct"));
    command.current_dir(root);
    if entry.proof == "canonical-check-rejection" {
        command.arg("--check");
    }
    if entry.proof == "canonical-file-write" {
        fs::write(
            root.join("target/conduit-filesystem-example.bin"),
            b"replaceable fixture\n",
        )
        .expect("file-write target is prepared");
        command.arg("--enable-file-write");
    }
    if entry.proof == "canonical-file-watch" {
        command.arg("--enable-file-watch");
    }
    if entry.proof == "canonical-storage-cache" {
        command.arg("--enable-storage-cache");
    }
    command.arg(&entry.path).output().expect("conduct executes")
}

fn invoke_http(root: &Path, entry: &Entry, request_path: &str) -> (std::process::Output, Vec<u8>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .current_dir(root)
        .arg(&entry.path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("HTTP conduct run starts");
    let mut stderr = BufReader::new(child.stderr.take().expect("HTTP stderr"));
    let mut diagnostics = String::new();
    let address = loop {
        let mut line = String::new();
        assert_ne!(
            stderr.read_line(&mut line).expect("HTTP status line"),
            0,
            "HTTP run exited before binding: {diagnostics}"
        );
        diagnostics.push_str(&line);
        if let Some(address) = line.trim().strip_prefix("CND-HTTP-BOUND ") {
            break address.to_owned();
        }
    };
    let mut stream = TcpStream::connect(address).expect("checked HTTP listener accepts");
    let request =
        format!("GET {request_path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .expect("HTTP request writes");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("HTTP response reads");
    let status = child.wait().expect("HTTP conduct run exits");
    stderr
        .read_to_string(&mut diagnostics)
        .expect("remaining HTTP diagnostics");
    let mut stdout = Vec::new();
    child
        .stdout
        .take()
        .expect("HTTP stdout")
        .read_to_end(&mut stdout)
        .expect("HTTP stdout reads");
    (
        std::process::Output {
            status,
            stdout,
            stderr: diagnostics.into_bytes(),
        },
        response,
    )
}

#[test]
fn every_checked_panel_has_one_verified_runnability_state() {
    let root = root();
    let inventory: Inventory = serde_json::from_str(
        &fs::read_to_string(root.join("examples/runnability.json"))
            .expect("runnability inventory exists"),
    )
    .expect("runnability inventory is valid");
    assert_eq!(inventory.schema, "conduit.panel-runnability");
    assert_eq!(
        inventory.states,
        ["runnable", "contract-only", "illustrative/unavailable"]
    );

    let mut checked_in = BTreeSet::new();
    for directory in ["examples", "fixtures", "conformance"] {
        collect_panels(&root, Path::new(directory), &mut checked_in);
    }
    let declared = inventory
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        declared.len(),
        inventory.entries.len(),
        "each panel has exactly one runnability declaration"
    );
    assert_eq!(declared, checked_in, "panel inventory has no gaps or drift");

    let http_source =
        fs::read_to_string(root.join("examples/http-loopback-once.panel")).expect("HTTP source");
    let mut registry = conduit_runtime::Registry::hosted_primitives();
    conduit_http::register_hosted_http_provider(&mut registry).expect("HTTP provider links");
    let installed = conduit_compile::InstalledProfile::observe_registry(&http_source, &registry)
        .expect("HTTP installed profile resolves");
    let candidate = installed
        .input
        .candidates
        .iter()
        .find(|candidate| candidate.implementation.semantic_contract.id == "net/http/serve-once")
        .expect("HTTP provider candidate");
    assert_eq!(
        candidate.implementation.required_authorities,
        ["sha256:4848484848484848484848484848484848484848484848484848484848484848"],
        "the installed provider requires the exact loopback-listen authority"
    );
    assert_eq!(candidate.authorities.len(), 1);
    assert_eq!(candidate.authorities[0].status, "active");
    assert_eq!(
        candidate.authorities[0].grant.id,
        "conduit.grant/http-loopback-listen"
    );
    let document = conduit_compile::compile_source(&http_source, &installed.input)
        .expect("HTTP exact plan compiles");
    let arena = bumpalo::Bump::new();
    let plan = document.as_plan(&arena).expect("HTTP exact plan loads");
    assert_eq!(plan.authorities.len(), 1);
    assert_eq!(
        plan.authorities[0].grant.id.as_str(),
        "conduit.grant/http-loopback-listen"
    );

    for entry in &inventory.entries {
        assert!(
            ["deterministic", "browser", "hosted", "embedded", "device"]
                .contains(&entry.profile.as_str()),
            "{} has an explicit host profile",
            entry.path
        );
        if entry.proof == "canonical-http-loopback" {
            let (output, response) = invoke_http(&root, entry, "/health");
            assert!(output.status.success(), "{} must run", entry.path);
            assert!(output.stdout.is_empty(), "{} has clean stdout", entry.path);
            assert!(
                response.starts_with(b"HTTP/1.1 200 OK\r\n"),
                "{} returns HTTP success",
                entry.path
            );
            assert!(
                response.ends_with(b"conduit http ready\n"),
                "{} returns the exact checked response",
                entry.path
            );
            assert!(
                String::from_utf8(output.stderr)
                    .expect("HTTP diagnostics are UTF-8")
                    .starts_with("CND-HTTP-BOUND 127.0.0.1:"),
                "{} publishes only its loopback binding",
                entry.path
            );
            let (missing_output, missing_response) = invoke_http(&root, entry, "/missing");
            assert!(
                missing_output.status.success(),
                "{} terminates cleanly after an unknown route",
                entry.path
            );
            assert!(
                missing_response.starts_with(b"HTTP/1.1 404 Not Found\r\n"),
                "{} rejects an unknown route deterministically",
                entry.path
            );
            continue;
        }
        let output = invoke(&root, entry);
        let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
        let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
        if entry.state == "runnable" {
            assert!(output.status.success(), "{} must run", entry.path);
            assert_eq!(
                stdout,
                entry.expected_stdout.as_deref().expect("runnable stdout"),
                "{} exact stdout",
                entry.path
            );
            assert_eq!(
                stderr,
                entry.expected_stderr.as_deref().unwrap_or_default(),
                "{} exact stderr",
                entry.path
            );
        } else {
            assert!(!output.status.success(), "{} must fail closed", entry.path);
            assert!(
                stderr.contains(
                    entry
                        .expected_diagnostic
                        .as_deref()
                        .expect("non-runnable diagnostic")
                ),
                "{} emits its structured diagnostic: {stderr}",
                entry.path
            );
            assert!(stdout.is_empty(), "{} has clean stdout", entry.path);
        }
    }

    let lessons: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("tour/lessons/current.json")).expect("Tour lessons exist"),
    )
    .expect("Tour lessons are valid");
    for lesson in lessons["lessons"].as_array().expect("lessons are listed") {
        if lesson["runnability"]["state"] != "runnable" {
            continue;
        }
        let id = lesson["id"].as_str().expect("lesson id");
        let path = std::env::temp_dir().join(format!(
            "conduit-tour-export-{}-{}.panel",
            std::process::id(),
            id.replace(['.', '/'], "-")
        ));
        fs::write(&path, lesson["source"].as_str().expect("lesson source"))
            .expect("exported lesson can be written");
        let checked = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg("--check")
            .arg(&path)
            .output()
            .expect("canonical check executes");
        assert!(checked.status.success(), "{id} exported source checks");
        let ran = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg(&path)
            .output()
            .expect("canonical default run executes");
        fs::remove_file(&path).expect("temporary export is removed");
        assert!(ran.status.success(), "{id} exported source runs");
        assert_eq!(
            String::from_utf8(ran.stdout).expect("lesson stdout is UTF-8"),
            lesson["expected_display"]
                .as_str()
                .expect("runnable display"),
            "{id} exported source projects the same display result"
        );
        assert_eq!(
            String::from_utf8(ran.stderr).expect("lesson stderr is UTF-8"),
            lesson["expected_stderr"].as_str().unwrap_or_default(),
            "{id} exported source has the declared canonical stderr"
        );
    }
}

#[test]
fn canonical_http_timeout_is_a_structured_terminal_failure() {
    let root = root();
    let source = fs::read_to_string(root.join("examples/http-loopback-once.panel"))
        .expect("HTTP example source")
        .replace("deadline_ms = \"5000\"", "deadline_ms = \"1\"");
    let panel =
        std::env::temp_dir().join(format!("conduit-http-timeout-{}.panel", std::process::id()));
    fs::write(&panel, source).expect("temporary HTTP panel writes");
    let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .current_dir(root)
        .arg(&panel)
        .output()
        .expect("HTTP timeout run executes");
    fs::remove_file(&panel).expect("temporary HTTP panel is removed");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostics = String::from_utf8(output.stderr).expect("HTTP diagnostics are UTF-8");
    assert!(diagnostics.starts_with("CND-HTTP-BOUND 127.0.0.1:"));
    assert!(
        diagnostics.contains("CND-HTTP-017"),
        "provider timeout remains a structured terminal reason: {diagnostics}"
    );
}

#[test]
fn filesystem_exact_plan_pins_provider_authority_resource_and_bounds() {
    let root = root();
    let source =
        fs::read_to_string(root.join("examples/filesystem-read.panel")).expect("read example");
    let mut registry = conduit_runtime::Registry::hosted_primitives();
    conduit_filesystem::register_hosted_file_read_provider(&mut registry)
        .expect("read provider links");
    let installed = conduit_compile::InstalledProfile::observe_registry(&source, &registry)
        .expect("filesystem installed profile resolves");
    let candidate = installed
        .input
        .candidates
        .iter()
        .find(|candidate| candidate.implementation.semantic_contract.id == "fs/read")
        .expect("filesystem read candidate");
    assert_eq!(candidate.implementation.id, "conduit/filesystem-linux-read");
    assert_eq!(candidate.authorities.len(), 1);
    assert_eq!(candidate.authorities[0].status, "active");
    assert_eq!(
        candidate.authorities[0].grant.id,
        "conduit.grant/filesystem-read"
    );
    assert_eq!(
        candidate.authorities[0].grant.resource_id,
        "conduit.resource/filesystem-example-read"
    );

    let document =
        conduit_compile::compile_source(&source, &installed.input).expect("exact plan compiles");
    let arena = bumpalo::Bump::new();
    let plan = document.as_plan(&arena).expect("exact plan loads");
    let node = plan
        .nodes
        .iter()
        .find(|node| node.contract.id.as_str() == "fs/read")
        .expect("read node is planned");
    assert_eq!(
        node.implementation.id.as_str(),
        "conduit/filesystem-linux-read"
    );
    assert_eq!(
        node.artifact.as_str(),
        "conduit/filesystem-linux-read-artifact"
    );
    assert_eq!(node.host.as_str(), "conduit/conduct-host");
    let profile = node.execution_profile.expect("execution profile is pinned");
    assert_eq!(profile.limits.max_input_bytes, 64 * 1024);
    assert_eq!(profile.limits.max_output_bytes, 64 * 1024);
    assert_eq!(profile.limits.max_pending_operations, 1);
    let authority = plan
        .authorities
        .iter()
        .find(|authority| authority.node == node.instance)
        .expect("read authority is planned");
    assert_eq!(authority.grant.id.as_str(), "conduit.grant/filesystem-read");
    assert_eq!(
        authority.binding.resource.id.as_str(),
        "conduit.resource/filesystem-example-read"
    );
    assert_eq!(
        authority.binding.resource.kind.as_str(),
        "conduit.resource/filesystem-file"
    );
    assert!(authority.commit_profile.is_some());
}

#[test]
fn storage_cache_exact_plan_pins_distinct_provider_resource_grant_and_bounds() {
    let root = root();
    let source =
        fs::read_to_string(root.join("examples/storage-cache.panel")).expect("cache example");
    let mut registry = conduit_runtime::Registry::hosted_primitives();
    conduit_cache::register_hosted_cache_provider(&mut registry).expect("cache provider links");
    let installed = conduit_compile::InstalledProfile::observe_registry(&source, &registry)
        .expect("cache installed profile resolves");

    for (contract, implementation, grant, resource) in [
        (
            "storage/cache/put",
            "conduit/storage-cache-put",
            "conduit.grant/storage-cache-put",
            "conduit.resource/storage-cache-example-put",
        ),
        (
            "storage/cache/get",
            "conduit/storage-cache-get",
            "conduit.grant/storage-cache-get",
            "conduit.resource/storage-cache-example-get",
        ),
    ] {
        let candidate = installed
            .input
            .candidates
            .iter()
            .find(|candidate| candidate.implementation.semantic_contract.id == contract)
            .expect("cache candidate");
        assert_eq!(candidate.implementation.id, implementation);
        assert_eq!(candidate.authorities.len(), 1);
        assert_eq!(candidate.authorities[0].status, "active");
        assert_eq!(candidate.authorities[0].grant.id, grant);
        assert_eq!(candidate.authorities[0].grant.resource_id, resource);
    }

    let document =
        conduit_compile::compile_source(&source, &installed.input).expect("exact plan compiles");
    let arena = bumpalo::Bump::new();
    let plan = document.as_plan(&arena).expect("exact plan loads");
    for (contract, implementation, grant, resource) in [
        (
            "storage/cache/put",
            "conduit/storage-cache-put",
            "conduit.grant/storage-cache-put",
            "conduit.resource/storage-cache-example-put",
        ),
        (
            "storage/cache/get",
            "conduit/storage-cache-get",
            "conduit.grant/storage-cache-get",
            "conduit.resource/storage-cache-example-get",
        ),
    ] {
        let node = plan
            .nodes
            .iter()
            .find(|node| node.contract.id.as_str() == contract)
            .expect("cache node is planned");
        assert_eq!(node.implementation.id.as_str(), implementation);
        assert_eq!(node.artifact.as_str(), "conduit/storage-cache-artifact");
        assert_eq!(node.host.as_str(), "conduit/conduct-host");
        let profile = node.execution_profile.expect("execution profile is pinned");
        assert_eq!(profile.limits.max_input_bytes, 64 * 1024);
        assert_eq!(profile.limits.max_output_bytes, 64 * 1024);
        assert_eq!(profile.limits.max_pending_operations, 1);
        let authority = plan
            .authorities
            .iter()
            .find(|authority| authority.node == node.instance)
            .expect("cache authority is planned");
        assert_eq!(authority.grant.id.as_str(), grant);
        assert_eq!(authority.binding.resource.id.as_str(), resource);
        assert_eq!(
            authority.binding.resource.kind.as_str(),
            "conduit.resource/evictable-blob-cache"
        );
        assert!(authority.commit_profile.is_some());
    }
}

#[test]
fn storage_cache_resolution_distinguishes_omission_staleness_capacity_and_grant() {
    let root = root();
    let source =
        fs::read_to_string(root.join("examples/storage-cache.panel")).expect("cache example");
    let absent = conduit_compile::InstalledProfile::observe_registry(
        &source,
        &conduit_runtime::Registry::hosted_primitives(),
    )
    .err()
    .expect("known cache contracts do not imply an installed provider");
    assert_eq!(absent.code, "CND-IMP-001");

    let mut registry = conduit_runtime::Registry::hosted_primitives();
    conduit_cache::register_hosted_cache_provider(&mut registry).expect("cache provider links");
    let installed = conduit_compile::InstalledProfile::observe_registry(&source, &registry)
        .expect("cache installed profile resolves");

    let mut stale = installed.input.clone();
    for candidate in &mut stale.candidates {
        if candidate
            .implementation
            .semantic_contract
            .id
            .starts_with("storage/")
        {
            candidate.host_report.valid_until_tick = 0;
        }
    }
    stale.seal().expect("stale fixture reseals exactly");
    assert_eq!(
        conduit_compile::compile_source(&source, &stale)
            .expect_err("stale reports cannot place cache nodes")
            .code(),
        "CND-CMP-006"
    );

    let mut insufficient = installed.input.clone();
    for candidate in &mut insufficient.candidates {
        if candidate.implementation.semantic_contract.id == "storage/cache/put" {
            candidate.host_report.available.memory_bytes = 1;
        }
    }
    insufficient
        .seal()
        .expect("insufficient-capacity fixture reseals exactly");
    assert_eq!(
        conduit_compile::compile_source(&source, &insufficient)
            .expect_err("insufficient capacity cannot place the put")
            .code(),
        "CND-CMP-006"
    );

    let denied_source = source.replace(
        "conduit.grant/storage-cache-put",
        "conduit.grant/storage-cache-denied",
    );
    let denied = conduit_compile::InstalledProfile::observe_registry(&denied_source, &registry)
        .err()
        .expect("a cache handle never substitutes for the required grant");
    assert_eq!(denied.code, "CND-CACHE-012");
}
