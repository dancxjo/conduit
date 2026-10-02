//! Explicit local proof through a real installed executable, without systemd activation.
use crate::durable_host::{
    bundle_digest, digest, fresh_identity, read_installation, ReleaseFile, RELEASE_SCHEMA,
};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

#[test]
#[ignore = "requires a freshly built product executable in CONDUIT_OWNER_SMOKE_BINARY"]
fn installed_cli_runs_and_recovers_same_body_on_new_boot() {
    let binary = std::env::var_os("CONDUIT_OWNER_SMOKE_BINARY").expect("product executable path");
    let root = std::env::temp_dir().join(fresh_identity("owner-cli-smoke", "installed"));
    let bundle = root.join("bundle");
    fs::create_dir_all(&bundle).unwrap();
    let bytes = fs::read(&binary).unwrap();
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
            "schema": RELEASE_SCHEMA, "source_identity": "local-owner-cli-smoke",
            "bundle_sha256": bundle_digest(&files), "files": files
        }))
        .unwrap(),
    )
    .unwrap();
    let state = root.join("state");
    let installed = Command::new(&binary)
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
    assert!(!state.join("runtime.json").exists());
    let installation = read_installation(&state.join("installation.json")).unwrap();
    let source = root.join("hello.conduit");
    fs::write(
        &source,
        "plot hello {\n show: presentation/text\n \"Hello.\" >> show\n}.",
    )
    .unwrap();
    let first = run(&installation.product_executable, &state, &source,
        b"{\"operation\":\"plan\"}\n{\"operation\":\"run\",\"maximum_millis\":1000}\n{\"operation\":\"close\"}\n");
    let last = first.last().unwrap();
    assert!(last["last_execution"]["output_utf8"]
        .as_str()
        .unwrap()
        .contains("Hello."));
    assert!(last["last_execution"]["play"]["active_play_id"].is_string());
    assert!(last["realization"].is_null());
    let second = run(
        &installation.product_executable,
        &state,
        &source,
        b"{\"operation\":\"inspect\"}\n{\"operation\":\"close\"}\n",
    );
    assert_eq!(
        second[0]["biography"]["body_id"],
        last["biography"]["body_id"]
    );
    assert_eq!(second[0]["host"]["host_id"], last["host"]["host_id"]);
    assert_ne!(second[0]["host"]["boot_id"], last["host"]["boot_id"]);
    assert_eq!(second[0]["last_execution"], last["last_execution"]);
    assert!(second[0]["realization"].is_null());
    fs::write(
        root.join("first.json"),
        serde_json::to_vec_pretty(&first).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("reopened.json"),
        serde_json::to_vec_pretty(&second).unwrap(),
    )
    .unwrap();
    println!("local installed CLI smoke evidence: {}", root.display());
}

fn run(binary: &str, state: &Path, source: &Path, commands: &[u8]) -> Vec<serde_json::Value> {
    let mut child = Command::new(binary)
        .args(["body", "own"])
        .arg(source)
        .arg("--state-dir")
        .arg(state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(commands).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
