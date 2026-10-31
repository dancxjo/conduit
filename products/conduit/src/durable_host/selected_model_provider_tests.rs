//! Real installed-selection lifecycle against a bounded synthetic loopback provider.
use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

struct Provider {
    endpoint: String,
    changed: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Provider {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let changed = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (changed_worker, stop_worker) = (changed.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut requests = 0;
            while !stop_worker.load(Ordering::Relaxed) && Instant::now() < deadline {
                match listener.accept() {
                    Ok((stream, _)) => {
                        requests += 1;
                        assert!(requests <= 24, "finite fixture request budget");
                        respond(stream, changed_worker.load(Ordering::Relaxed));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => panic!("fixture accept: {e}"),
                }
            }
        });
        Self {
            endpoint,
            changed,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let result = thread.join();
            if !std::thread::panicking() {
                result.unwrap();
            }
        }
    }
}
fn respond(mut stream: TcpStream, changed: bool) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut bytes = Vec::with_capacity(8192);
    let mut block = [0; 1024];
    loop {
        let count = stream.read(&mut block).unwrap();
        assert!(count > 0, "complete fixture request");
        assert!(bytes.len() + count <= 8192, "finite request extent");
        bytes.extend_from_slice(&block[..count]);
        if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&bytes[..end]).unwrap();
            let length = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            assert!(length <= 4096);
            if bytes.len() >= end + 4 + length {
                break;
            }
        }
    }
    let request = std::str::from_utf8(&bytes).unwrap();
    let route = request
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap();
    let body = match route {
        "/api/tags" => serde_json::json!({"models":[{
            "name":"fixture:latest", "digest":format!("sha256:{}", if changed {"b"} else {"a"}.repeat(64)),
            "size":1024,"details":{"family":"fixture","parameter_size":"tiny","quantization_level":"Q4"}
        }]}),
        "/api/show" => serde_json::json!({"model_info":{
            "general.architecture":"fixture","fixture.context_length":4096
        },"capabilities":["completion"]}),
        "/api/generate" => serde_json::json!({"response":"ready","done":true}),
        _ => panic!("unexpected provider route {route}"),
    }.to_string();
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
}
fn select(endpoint: &str, profile: LocalModelKindProfile) -> Selection {
    let Change::Replace(selection) = change(InstalledModelOptions {
        selected_model: Some("fixture".into()),
        model_endpoint: Some(endpoint.into()),
        model_memory_mib: Some(1),
        model_profiles: vec![profile],
        ..Default::default()
    })
    .unwrap() else {
        panic!("explicit installed selection")
    };
    serde_json::from_slice(&serde_json::to_vec(&selection).unwrap()).unwrap()
}
fn installed_kind_names(selection: &Selection) -> Vec<String> {
    let adapter = selection.initialize().unwrap();
    assert_eq!(adapter.offer(), &selection.reviewed_offer);
    let host = conduit_std_host::StdHost::new_with_local_model(
        conduit_std_host::StdHostConfig {
            host_id: "host/model-fixture".into(),
            boot_id: "boot/fresh-model-fixture".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        conduit_std_host::StdHostComposition::minimal(),
        Box::new(adapter),
    )
    .unwrap();
    host.advertisement()
        .capabilities
        .iter()
        .filter(|offer| {
            offer.implementation.implementation_id.as_str()
                == conduit_ai::LOCAL_MODEL_IMPLEMENTATION
        })
        .map(|offer| offer.kind_id.as_str().to_owned())
        .collect()
}

#[test]
fn installed_selection_reobserves_exact_profiles_and_rejects_provider_identity_drift() {
    const CHILD: &str = "CONDUIT_SELECTED_MODEL_FIXTURE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // PATH belongs only to this child test process, never the parallel test runner.
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "conduit-model-profile-fixture-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let executable = dir.join("ollama");
        std::fs::write(
            &executable,
            b"#!/bin/sh\n[ \"$1\" = \"--version\" ] || exit 1\nprintf 'fixture-ollama-1\\n'\n",
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut paths = vec![dir.clone()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "durable_host::selected_model::provider_tests::installed_selection_reobserves_exact_profiles_and_rejects_provider_identity_drift", "--nocapture"])
            .env(CHILD, "1").env("PATH", std::env::join_paths(paths).unwrap()).output().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
        assert!(
            String::from_utf8_lossy(&result.stdout).contains("1 passed; 0 failed"),
            "the exact fixture child must execute its test: {}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            result.status.success(),
            "fixture child failed: {}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let provider = Provider::start();
    let stream = select(&provider.endpoint, LocalModelKindProfile::StreamGenerate);
    assert_eq!(
        installed_kind_names(&stream),
        vec![LocalModelKindProfile::StreamGenerate.kind()]
    );
    let generate = select(&provider.endpoint, LocalModelKindProfile::Generate);
    assert_eq!(
        generate.reviewed_offer.supported_profiles,
        vec![
            LocalModelKindProfile::Generate,
            LocalModelKindProfile::GenerateFlow
        ]
    );
    let kinds = installed_kind_names(&generate);
    assert_eq!(kinds.len(), 2);
    assert!(kinds.contains(&LocalModelKindProfile::Generate.kind().into()));
    assert!(kinds.contains(&LocalModelKindProfile::GenerateFlow.kind().into()));
    provider.changed.store(true, Ordering::Relaxed);
    let Err(error) = stream.initialize() else {
        panic!("changed provider must refuse fresh Boot")
    };
    assert!(error.contains("provider changed"), "{error}");
}
