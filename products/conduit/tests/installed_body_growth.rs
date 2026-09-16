//! Installed Body growth across independent durable-Host product processes.

#![cfg(unix)]

use conduit_body::{Body, BodyBiographyEvidence, BodyMembership};
use conduit_core::SignId;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct RunningHost(Child);

impl Drop for RunningHost {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn two_installed_processes_complete_pipeable_body_admission_without_creating_a_play() {
    let root = unique_root();
    let owner = root.join("owner");
    let joining = root.join("joining");
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&joining).unwrap();
    seed_installation(&owner, true);
    seed_installation(&joining, false);

    let _owner_service = start_host(&owner);
    let _joining_service = start_host(&joining);
    wait_for_runtime(&owner);
    wait_for_runtime(&joining);

    let invitation = product(&[
        "body",
        "invite",
        "--state-dir",
        path(&owner),
        "--ttl-seconds",
        "60",
    ]);
    assert_success(&invitation, "issue invitation");
    let invitation: Value = serde_json::from_slice(&invitation.stdout).unwrap();
    assert_eq!(invitation["schema"], "conduit.body/spawn-invitation@1");

    let request = product_with_stdin(
        &[
            "body",
            "accept",
            "-",
            "--state-dir",
            path(&joining),
            "--authorize-join",
        ],
        &serde_json::to_vec(&invitation).unwrap(),
    );
    assert_success(&request, "accept invitation");
    let request: Value = serde_json::from_slice(&request.stdout).unwrap();
    assert_eq!(request["schema"], "conduit.body/spawn-admission-request@1");
    assert_eq!(request["membership_admitted"], false);
    assert_eq!(request["plan_created"], false);
    assert_eq!(request["play_created"], false);

    let receipt = product_with_stdin(
        &[
            "body",
            "admit",
            "-",
            "--state-dir",
            path(&owner),
            "--authorize-admission",
        ],
        &serde_json::to_vec(&request).unwrap(),
    );
    assert_success(&receipt, "admit Host");
    let receipt: Value = serde_json::from_slice(&receipt.stdout).unwrap();
    assert_eq!(receipt["schema"], "conduit.body/spawn-admission-receipt@1");
    assert_eq!(receipt["membership_admitted"], true);
    assert_eq!(receipt["current_offers_available"], true);
    assert_eq!(receipt["plan_created"], false);
    assert_eq!(receipt["play_created"], false);

    let owner_status = product(&["body", "status", "--state-dir", path(&owner), "--json"]);
    assert_success(&owner_status, "inspect admitted Body");
    let owner_status: Value = serde_json::from_slice(&owner_status.stdout).unwrap();
    assert_eq!(owner_status["body_id"], invitation["claim"]["body_id"]);
    assert_eq!(owner_status["presence"], "current");
    assert_eq!(owner_status["member_count"], 1);
    assert_eq!(owner_status["present_member_count"], 1);
    assert_eq!(owner_status["plan_created"], false);
    assert_eq!(owner_status["play_created"], false);

    fs::remove_dir_all(root).unwrap();
}

fn seed_installation(state: &Path, owns_body: bool) {
    let executable = state.join("installed-conduit");
    fs::write(&executable, b"reviewed installed product image").unwrap();
    let executable_sha = digest(&fs::read(&executable).unwrap());
    let body_state = if owns_body {
        let body = Body::born(
            "source/installed-process-proof".into(),
            "checked/installed-process-proof".into(),
            1,
            SignId::from("sign/installed-process-proof/born"),
        )
        .unwrap();
        let biography = BodyBiographyEvidence::born(
            body.clone(),
            BodyMembership::new(body.body_id.clone()).unwrap(),
            "Independent installed process proof".into(),
        )
        .unwrap();
        let body_dir = state.join("body");
        fs::create_dir_all(&body_dir).unwrap();
        let biography_path = body_dir.join("biography.json");
        let bytes = serde_json::to_vec_pretty(&biography).unwrap();
        fs::write(&biography_path, &bytes).unwrap();
        Some(json!({
            "body_id": body.body_id.as_str(),
            "biography_sha256": digest(&bytes),
            "biography_path": biography_path,
        }))
    } else {
        None
    };
    fs::write(state.join("control.token"), [37_u8; 32]).unwrap();
    fs::write(
        state.join("installation.json"),
        serde_json::to_vec_pretty(&json!({
            "schema": "conduit.install/durable-host@1",
            "host_id": format!("host/installed/{}", state.file_name().unwrap().to_string_lossy()),
            "release_source_identity": "commit:installed-process-proof",
            "release_bundle_sha256": executable_sha,
            "product_executable": executable,
            "body_state": body_state,
        }))
        .unwrap(),
    )
    .unwrap();
}

fn start_host(state: &Path) -> RunningHost {
    RunningHost(
        Command::new(env!("CARGO_BIN_EXE_conduit"))
            .args(["host", "service", "run", "--state-dir", path(state)])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    )
}

fn wait_for_runtime(state: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if state.join("runtime.json").is_file() && state.join("control.sock").exists() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "durable Host did not expose current runtime truth at {}",
        state.display()
    );
}

fn product(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args(arguments)
        .output()
        .unwrap()
}

fn product_with_stdin(arguments: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn assert_success(output: &Output, operation: &str) {
    assert!(
        output.status.success(),
        "{operation} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn unique_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "conduit-installed-growth-{}-{nonce}",
        std::process::id()
    ))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn path(value: &Path) -> &str {
    value.to_str().unwrap()
}
