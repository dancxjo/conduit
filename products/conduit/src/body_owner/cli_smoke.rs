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
    let native_invitation = Command::new(&installation.product_executable)
        .args(["body", "invite", "--state-dir"])
        .arg(&state)
        .output()
        .unwrap();
    assert!(native_invitation.status.success());
    let native_invitation: PortableInvitation =
        serde_json::from_slice(&native_invitation.stdout).unwrap();
    let native = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("host/installed-conduitos-guest"),
        boot_id: BootId::from("boot/installed-conduitos-guest/1"),
        offer_generation: OfferGeneration(1),
    });
    let native_advertisement = native.advertisement().clone();
    let native_secret = SpawnInvitationSecret::from_csprng_bytes(native_invitation.secret).unwrap();
    let native_signature = native_secret.sign(&native_invitation.claim.signing_transcript(
        &native_advertisement.host_id,
        &native_advertisement.boot_id,
        native_advertisement.offer_generation,
    ));
    let native_observation = serde_json::json!({
        "schema":"conduit.conduitos/serial-spawn-observation@1", "protocol":1,
        "spore_id":"spore/installed-native-smoke", "image_id":"image/installed-native-smoke",
        "advertisement":native_advertisement,
        "invitation_id":native_invitation.claim.invitation_id,
        "body_id":native_invitation.claim.body_id,
        "host_id":native_advertisement.host_id, "boot_id":native_advertisement.boot_id,
        "nonce":native_invitation.claim.nonce, "signature":native_signature.to_vec(),
        "expiry_checked_by_body":true, "membership_claimed":false
    });
    let native_commands = format!(
        "{}\n{{\"operation\":\"close\"}}\n",
        serde_json::json!({"operation":"admit-native-observation",
            "expected_host_id":native_advertisement.host_id,
            "observation":native_observation})
    );
    let native_admitted = run(
        &installation.product_executable,
        &state,
        &source,
        native_commands.as_bytes(),
    );
    let native_receipt: conduit_body::PortableAdmissionReceipt = serde_json::from_value(
        native_admitted
            .iter()
            .find(|item| item["schema"] == conduit_body::SPAWN_ADMISSION_RECEIPT_SCHEMA)
            .unwrap()
            .clone(),
    )
    .unwrap();
    assert_eq!(
        native_receipt.credential.host_id,
        native_advertisement.host_id
    );
    assert_eq!(
        native_receipt.credential.boot_id,
        native_advertisement.boot_id
    );
    assert!(!native_receipt.plan_created && !native_receipt.play_created);
    assert_eq!(
        native_admitted.last().unwrap()["biography"]["membership"]["parts"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let native_replay = run(
        &installation.product_executable,
        &state,
        &source,
        native_commands.as_bytes(),
    );
    assert!(native_replay
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
    fs::write(
        root.join("native-observation-admission.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"receipt":native_receipt,
            "owner":native_admitted.last(), "replay":native_replay.last()}))
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
