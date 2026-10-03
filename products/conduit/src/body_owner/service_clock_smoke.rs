//! Real installed-service entrance for the retained clock Play.
use crate::durable_host::{bundle_digest, digest, fresh_identity, ReleaseFile, RELEASE_SCHEMA};
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Service(Child);
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn call(binary: &Path, state: &Path, command: &str, extra: &[&str]) -> std::process::Output {
    Command::new(binary)
        .args(["body", command, "--state-dir"])
        .arg(state)
        .args(extra)
        .output()
        .unwrap()
}

fn status(binary: &Path, state: &Path) -> serde_json::Value {
    let output = call(binary, state, "status", &["--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
#[ignore = "requires a freshly built product executable in CONDUIT_OWNER_SMOKE_BINARY"]
fn installed_service_start_inspect_lull_and_restart_clock() {
    let binary = std::env::var_os("CONDUIT_OWNER_SMOKE_BINARY").expect("product executable path");
    let binary = Path::new(&binary);
    let root = std::env::temp_dir().join(fresh_identity("owner-clock-smoke", "installed"));
    let bundle = root.join("bundle");
    fs::create_dir_all(&bundle).unwrap();
    let bytes = fs::read(binary).unwrap();
    fs::write(bundle.join("conduit-linux-x86_64"), &bytes).unwrap();
    let files = vec![ReleaseFile {
        path: "conduit-linux-x86_64".into(),
        bytes: bytes.len() as u64,
        sha256: digest(&bytes),
    }];
    let manifest = bundle.join("release.json");
    fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!({
            "schema":RELEASE_SCHEMA, "source_identity":"local-service-clock-smoke",
            "bundle_sha256":bundle_digest(&files), "files":files,
        }))
        .unwrap(),
    )
    .unwrap();
    let state = root.join("state");
    let installed = Command::new(binary)
        .args(["host", "service", "install"])
        .arg(&manifest)
        .arg("--state-dir")
        .arg(&state)
        .arg("--no-start")
        .output()
        .unwrap();
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    let source = root.join("clock.conduit");
    fs::write(
        &source,
        include_str!("../../../../plots/clock/main.conduit"),
    )
    .unwrap();
    let born = Command::new(binary)
        .args(["body", "own"])
        .arg(&source)
        .arg("--state-dir")
        .arg(&state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
        .wait_with_output()
        .unwrap();
    assert!(born.status.success());

    let mut service = Service(
        Command::new(binary)
            .args(["host", "service", "run", "--state-dir"])
            .arg(&state)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !state.join("control.sock").exists() {
        assert!(Instant::now() < deadline, "service control did not start");
        thread::sleep(Duration::from_millis(10));
    }
    let initial = status(binary, &state);
    let body_id = initial["biography"]["body_id"].clone();
    let first_boot = initial["host"]["boot_id"].clone();
    let started = call(binary, &state, "start", &["--maximum-millis", "10000"]);
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&started.stdout).unwrap()["play_state"],
        "preparing"
    );
    let live = loop {
        let truth = status(binary, &state);
        if truth["realization"]["play"]["active_play_id"].is_string() {
            break truth;
        }
        assert!(Instant::now() < deadline, "service clock did not start");
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(live["biography"]["body_id"], body_id);
    assert_eq!(live["host"]["boot_id"], first_boot);
    let play = live["realization"]["play"]["active_play_id"].clone();
    thread::sleep(Duration::from_millis(2200));
    let lull = call(binary, &state, "lull", &[]);
    assert!(
        lull.status.success(),
        "{}",
        String::from_utf8_lossy(&lull.stderr)
    );
    let lull: serde_json::Value = serde_json::from_slice(&lull.stdout).unwrap();
    assert_eq!(lull["active_play_id"], play);
    assert_eq!(lull["play_state"], "stop-requested");
    let stopped = loop {
        let truth = status(binary, &state);
        if truth["realization"].is_null() {
            break truth;
        }
        assert!(Instant::now() < deadline, "service clock did not lull");
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(stopped["biography"]["body_id"], body_id);
    assert_eq!(stopped["last_execution"]["play"]["active_play_id"], play);
    assert!(stopped["last_execution"]["output_utf8"]
        .as_str()
        .unwrap()
        .contains("tick sequence="));
    drop(service);
    let _ = fs::remove_file(state.join("control.sock"));
    service = Service(
        Command::new(binary)
            .args(["host", "service", "run", "--state-dir"])
            .arg(&state)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let restart_deadline = Instant::now() + Duration::from_secs(5);
    let restarted = loop {
        if state.join("control.sock").exists() {
            if let Ok(output) = Command::new(binary)
                .args(["body", "status", "--state-dir"])
                .arg(&state)
                .arg("--json")
                .output()
            {
                if output.status.success() {
                    break serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap();
                }
            }
        }
        assert!(Instant::now() < restart_deadline, "service did not restart");
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(restarted["biography"]["body_id"], body_id);
    assert_ne!(restarted["host"]["boot_id"], first_boot);
    assert!(restarted["realization"].is_null());
    drop(service);
    fs::remove_dir_all(root).unwrap();
}
