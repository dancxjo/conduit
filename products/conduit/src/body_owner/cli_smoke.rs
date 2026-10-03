//! Explicit local proof through a real installed executable, without systemd activation.
use crate::durable_host::{
    bundle_digest, digest, fresh_identity, read_installation, ReleaseFile, RELEASE_SCHEMA,
};
use conduit_body::{PortableInvitation, PortableSpawnAdmissionRequest, SpawnInvitationSecret};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostConfig};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    thread,
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
    let standalone = Command::new(&installation.product_executable)
        .args(["body", "invite", "--state-dir"])
        .arg(&state)
        .output()
        .unwrap();
    assert!(
        standalone.status.success(),
        "{}",
        String::from_utf8_lossy(&standalone.stderr)
    );
    let standalone_invitation: PortableInvitation =
        serde_json::from_slice(&standalone.stdout).unwrap();
    let issued = run(
        &installation.product_executable,
        &state,
        &source,
        b"{\"operation\":\"invite\",\"ttl_seconds\":60}\n{\"operation\":\"close\"}\n",
    );
    let invitation: PortableInvitation = serde_json::from_value(
        issued
            .iter()
            .find(|item| item["schema"] == conduit_body::INVITATION_SCHEMA)
            .unwrap()
            .clone(),
    )
    .unwrap();
    assert_ne!(
        invitation.claim.invitation_id,
        standalone_invitation.claim.invitation_id
    );
    let guest = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("host/installed-native-guest"),
        boot_id: BootId::from("boot/installed-native-guest/1"),
        offer_generation: OfferGeneration(1),
    });
    let advertisement = guest.advertisement().clone();
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: advertisement.clone(),
        nonce: invitation.claim.nonce,
        signature: secret
            .sign(&invitation.claim.signing_transcript(
                &advertisement.host_id,
                &advertisement.boot_id,
                advertisement.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let commands = format!(
        "{}\n{{\"operation\":\"close\"}}\n",
        serde_json::json!({"operation":"admit-invited", "expected_host_id":advertisement.host_id,
            "request":request.clone()})
    );
    let admitted = run(
        &installation.product_executable,
        &state,
        &source,
        commands.as_bytes(),
    );
    let receipt: conduit_body::PortableAdmissionReceipt = serde_json::from_value(
        admitted
            .iter()
            .find(|item| item["schema"] == conduit_body::SPAWN_ADMISSION_RECEIPT_SCHEMA)
            .unwrap()
            .clone(),
    )
    .unwrap();
    receipt.validate_against(&request).unwrap();
    assert_eq!(
        admitted.last().unwrap()["biography"]["membership"]["parts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let replay = run(
        &installation.product_executable,
        &state,
        &source,
        commands.as_bytes(),
    );
    assert!(replay
        .iter()
        .any(|item| item["schema"] == "conduit.body/owner-refusal@1"
            && item["message"].as_str().unwrap().contains("Replay")));
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
    fs::write(
        root.join("native-admission.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"receipt":receipt, "owner":admitted.last(),
            "replay":replay.last()}),
        )
        .unwrap(),
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
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let output_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let error_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let write = child.stdin.take().unwrap().write_all(commands);
    let status = child.wait().unwrap();
    let output = output_reader.join().unwrap();
    let errors = error_reader.join().unwrap();
    assert!(
        write.is_ok(),
        "owner input failed: {write:?}; status {status}; stderr {}; stdout {}",
        String::from_utf8_lossy(&errors),
        String::from_utf8_lossy(&output[output.len().saturating_sub(2048)..])
    );
    assert!(status.success(), "{}", String::from_utf8_lossy(&errors));
    String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
