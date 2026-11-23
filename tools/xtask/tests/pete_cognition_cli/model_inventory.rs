//! Fake inventory proves refusal ordering without contacting a real model/device.
use std::{
    io::{Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

struct Fixture {
    root: PathBuf,
    endpoint: String,
    server: Option<thread::JoinHandle<Vec<String>>>,
}

impl Fixture {
    fn inventory(model_name: &str, model_bytes: u64, completion: bool, requests: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let root = std::env::temp_dir().join(format!(
            "conduit-pete-model-cli-{}-{port}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let executable = root.join("ollama");
        std::fs::write(
            &executable,
            b"#!/bin/sh\nprintf 'ollama version is fixture\\n'\n",
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let tags = serde_json::json!({"models": [{
            "name": model_name, "digest": "a".repeat(64), "size": model_bytes,
            "details": {"family": "fixture", "parameter_size": "fixture", "quantization_level": "fixture"}
        }]}).to_string();
        let show = serde_json::json!({
            "model_info": {"general.architecture": "fixture", "fixture.context_length": 4096},
            "capabilities": if completion {vec!["completion"]} else {vec![]},
        })
        .to_string();
        let warmup = serde_json::json!({
            "response": "fixture-ready", "done": true,
            "prompt_eval_count": 1, "eval_count": 1,
        })
        .to_string();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut observed = Vec::new();
            while observed.len() < requests {
                let (mut socket, _) = match listener.accept() {
                    Ok(socket) => socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "missing fixture inventory request"
                        );
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let request = read_request(&mut socket);
                let first = request.lines().next().unwrap().to_owned();
                let body = if first.starts_with("GET /api/tags ") {
                    &tags
                } else if first.starts_with("POST /api/show ") {
                    &show
                } else if requests == 3 && first.starts_with("POST /api/generate ") {
                    &warmup
                } else {
                    panic!("model invocation or unexpected request: {first}");
                };
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                observed.push(first);
            }
            observed
        });
        Self {
            root,
            endpoint: format!("http://127.0.0.1:{port}"),
            server: Some(server),
        }
    }

    fn command(&self, cadenced: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        let path = std::env::join_paths(std::iter::once(self.root.clone()).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .unwrap();
        command.env("PATH", path).args([
            "prove",
            "pete",
            "std-cognition",
            "--model",
            "selected:fixture",
            "--model-endpoint",
            &self.endpoint,
            "--serial-path",
            "/conduit/no-such-device",
            "--utterance",
            "What is my battery state?",
        ]);
        if cadenced {
            command.args(["--reflection-after", "1"]);
        }
        command
    }

    fn finish(mut self, requests: usize) {
        assert_eq!(self.server.take().unwrap().join().unwrap().len(), requests);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn read_request(socket: &mut impl Read) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut buffer = [0; 1024];
        let count = socket.read(&mut buffer).unwrap();
        assert_ne!(count, 0);
        bytes.extend_from_slice(&buffer[..count]);
        assert!(bytes.len() <= 8192);
        if let Some(header_end) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            let header = std::str::from_utf8(&bytes[..header_end]).unwrap();
            let content_length = header
                .lines()
                .find_map(|line| {
                    line.split_once(':').and_then(|(name, value)| {
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                })
                .unwrap_or(0);
            if bytes.len() >= header_end + 4 + content_length {
                return String::from_utf8(bytes).unwrap();
            }
        }
    }
}

#[test]
fn discovered_models_refuse_before_serial_or_inference_when_incompatible() {
    for cadenced in [false, true] {
        for (name, bytes, completion, arguments, error, requests) in [
            (
                "other:fixture",
                1,
                true,
                vec![],
                "selected model is not already local",
                1,
            ),
            (
                "selected:fixture",
                1,
                true,
                vec![
                    "--expected-model-digest",
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                ],
                "digest mismatch",
                2,
            ),
            (
                "selected:fixture",
                2 * 1024 * 1024,
                true,
                vec!["--model-memory-mib", "1"],
                "local Ollama offer validation",
                2,
            ),
            (
                "selected:fixture",
                1,
                false,
                vec![],
                "does not advertise completion",
                2,
            ),
        ] {
            let fixture = Fixture::inventory(name, bytes, completion, requests);
            let output = fixture.command(cadenced).args(arguments).output().unwrap();
            assert!(!output.status.success());
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains(error), "{stderr}");
            assert!(!stderr.contains("open Create serial device"), "{stderr}");
            fixture.finish(requests);
        }
    }
}

#[test]
fn initialized_configuration_receipt_precedes_serial_acquisition() {
    for cadenced in [false, true] {
        let fixture = Fixture::inventory("selected:fixture", 1, true, 3);
        let receipt = fixture.root.join("model.json");
        let output = fixture
            .command(cadenced)
            .args([
                "--model-memory-mib",
                "1",
                "--expected-model-digest",
                &"a".repeat(64),
            ])
            .arg("--model-receipt-out")
            .arg(&receipt)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("open Create serial device"), "{stderr}");
        assert!(stderr.contains("CONDUIT_PETE_MODEL_EVIDENCE="), "{stderr}");
        let recorded: serde_json::Value =
            serde_json::from_slice(&std::fs::read(receipt).unwrap()).unwrap();
        assert_eq!(recorded["schema"], "conduit.pete/model-initialization@1");
        assert_eq!(recorded["endpoint"], fixture.endpoint);
        assert_eq!(
            recorded["discovery"]["model_content_identity"],
            "a".repeat(64)
        );
        assert_eq!(
            recorded["initialized_offer"]["identity"]["model_name"],
            "selected:fixture"
        );
        assert_eq!(
            recorded["initialized_offer"]["limits"]["admitted_memory_mib"],
            1
        );
        fixture.finish(3);
    }
}
