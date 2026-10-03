//! Installed body growth across independent durable-Host product processes.

#![cfg(unix)]

use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyMembership, MembershipProofId,
    PartId,
};
use conduit_core::{BootId, HostId, OfferGeneration, SignId};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, TcpListener},
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
fn routed_invitation_joins_two_installed_hosts_without_manual_protocol_phases() {
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

    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let certificate = root.join("owner-certificate.pem");
    let private_key = root.join("owner-private-key.pem");
    fs::write(&certificate, cert.pem()).unwrap();
    fs::write(&private_key, signing_key.serialize_pem()).unwrap();
    let reservation = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let bind = format!("0.0.0.0:{port}");
    let public_url = format!("wss://127.0.0.1:{port}/body-admission");
    let mut owner_route = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "body",
            "invite",
            "--state-dir",
            path(&owner),
            "--ttl-seconds",
            "30",
            "--route-bind",
            &bind,
            "--route-url",
            &public_url,
            "--route-tls-cert",
            path(&certificate),
            "--route-tls-key",
            path(&private_key),
            "--authorize-route",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut invitation = String::new();
    BufReader::new(owner_route.stdout.take().unwrap())
        .read_line(&mut invitation)
        .unwrap();
    let invitation: Value = serde_json::from_str(&invitation).unwrap();
    assert_eq!(invitation["schema"], "conduit.body/spawn-invitation@2");
    assert_eq!(
        invitation["rendezvous"]["body_id"],
        invitation["claim"]["body_id"]
    );
    assert_eq!(
        invitation["rendezvous"]["invitation_id"],
        invitation["claim"]["invitation_id"]
    );
    let invitation_path = root.join("routed-invitation.json");
    fs::write(&invitation_path, serde_json::to_vec(&invitation).unwrap()).unwrap();

    let mut expired = invitation.clone();
    expired["claim"]["expires_at_millis"] = json!(0);
    let expired_path = root.join("expired-routed-invitation.json");
    fs::write(&expired_path, serde_json::to_vec(&expired).unwrap()).unwrap();
    assert_failure_contains(
        &product(&[
            "body",
            "join",
            path(&expired_path),
            "--state-dir",
            path(&joining),
            "--authorize-join",
        ]),
        "Expired",
        "expired routed invitation",
    );

    let mut wrong_owner = invitation.clone();
    wrong_owner["rendezvous"]["body_id"] = json!("body/wrong-owner");
    let wrong_owner_path = root.join("wrong-owner-routed-invitation.json");
    fs::write(&wrong_owner_path, serde_json::to_vec(&wrong_owner).unwrap()).unwrap();
    assert_failure_contains(
        &product(&[
            "body",
            "join",
            path(&wrong_owner_path),
            "--state-dir",
            path(&joining),
            "--authorize-join",
        ]),
        "lost its exact invitation identity",
        "wrong owner identity",
    );

    let unreachable_reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let unreachable_port = unreachable_reservation.local_addr().unwrap().port();
    drop(unreachable_reservation);
    let mut unreachable = invitation.clone();
    unreachable["rendezvous"]["candidates"][0]["reachability"] =
        json!(format!("wss://127.0.0.1:{unreachable_port}/body-admission"));
    let unreachable_path = root.join("unreachable-routed-invitation.json");
    fs::write(&unreachable_path, serde_json::to_vec(&unreachable).unwrap()).unwrap();
    assert_failure_contains(
        &product(&[
            "body",
            "join",
            path(&unreachable_path),
            "--state-dir",
            path(&joining),
            "--authorize-join",
        ]),
        "owner-unreachable",
        "unreachable owner route",
    );
    let interrupted_installation: Value =
        serde_json::from_slice(&fs::read(joining.join("installation.json")).unwrap()).unwrap();
    assert!(interrupted_installation["joined_body_state"].is_null());
    assert!(!joining.join("body/membership-credential.json").exists());

    let joined = product(&[
        "body",
        "join",
        path(&invitation_path),
        "--state-dir",
        path(&joining),
        "--authorize-join",
    ]);
    assert_success(&joined, "join through exact owner route");
    let route_result = owner_route.wait_with_output().unwrap();
    assert!(
        route_result.status.success(),
        "owner route failed: {}",
        String::from_utf8_lossy(&route_result.stderr)
    );

    let joining_status = product(&["body", "status", "--state-dir", path(&joining), "--json"]);
    assert_success(&joining_status, "inspect retained joined membership");
    let joining_status: Value = serde_json::from_slice(&joining_status.stdout).unwrap();
    assert_eq!(joining_status["body_id"], invitation["claim"]["body_id"]);
    assert_eq!(joining_status["membership"], "admitted");
    assert_eq!(joining_status["plan_created"], false);
    assert_eq!(joining_status["play_created"], false);

    fs::remove_dir_all(root).unwrap();
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

    let mut tampered_request = request.clone();
    let first_signature_byte = tampered_request["signature"][0].as_u64().unwrap();
    tampered_request["signature"][0] = json!((first_signature_byte + 1) % 256);
    assert_failure_contains(
        &product_with_stdin(
            &[
                "body",
                "admit",
                "-",
                "--state-dir",
                path(&owner),
                "--authorize-admission",
            ],
            &serde_json::to_vec(&tampered_request).unwrap(),
        ),
        "InvalidProof",
        "tampered admission request",
    );

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

    let interrupted_installation: Value =
        serde_json::from_slice(&fs::read(joining.join("installation.json")).unwrap()).unwrap();
    assert!(interrupted_installation["joined_body_state"].is_null());
    assert!(joining.join("body/pending-join.json").is_file());
    assert!(!joining.join("body/membership-credential.json").exists());

    let mut tampered_receipt = receipt.clone();
    tampered_receipt["host_advertisement"]["boot_id"] = json!("boot/tampered");
    let tampered_receipt_path = root.join("tampered-receipt.json");
    fs::write(
        &tampered_receipt_path,
        serde_json::to_vec(&tampered_receipt).unwrap(),
    )
    .unwrap();
    assert_failure_contains(
        &product(&[
            "body",
            "complete-join",
            path(&tampered_receipt_path),
            "--state-dir",
            path(&joining),
            "--authorize-membership",
        ]),
        "lost its exact pending join identity",
        "tampered admission receipt",
    );
    assert!(joining.join("body/pending-join.json").is_file());
    assert!(!joining.join("body/membership-credential.json").exists());

    let receipt_path = root.join("receipt.json");
    fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    let completed = product(&[
        "body",
        "complete-join",
        path(&receipt_path),
        "--state-dir",
        path(&joining),
        "--authorize-membership",
    ]);
    assert_success(&completed, "retain exact admission receipt");
    assert!(!joining.join("body/pending-join.json").exists());
    assert!(joining.join("body/membership-credential.json").is_file());

    assert_failure_contains(
        &product_with_stdin(
            &[
                "body",
                "admit",
                "-",
                "--state-dir",
                path(&owner),
                "--authorize-admission",
            ],
            &serde_json::to_vec(&request).unwrap(),
        ),
        "Replay",
        "replayed admission request",
    );

    let owner_status = product(&["body", "status", "--state-dir", path(&owner), "--json"]);
    assert_success(&owner_status, "inspect admitted body");
    let owner_status: Value = serde_json::from_slice(&owner_status.stdout).unwrap();
    assert_eq!(
        owner_status["biography"]["body_id"],
        invitation["claim"]["body_id"]
    );
    assert_eq!(owner_status["remote_carrier_availability"], "unobserved");
    assert_eq!(
        owner_status["biography"]["membership"]["parts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let joining_status = product(&["body", "status", "--state-dir", path(&joining), "--json"]);
    assert_success(&joining_status, "inspect completed joined membership");
    let joining_status: Value = serde_json::from_slice(&joining_status.stdout).unwrap();
    assert_eq!(joining_status["body_id"], invitation["claim"]["body_id"]);
    assert_eq!(joining_status["membership"], "admitted");
    assert_eq!(joining_status["plan_created"], false);
    assert_eq!(joining_status["play_created"], false);

    fs::remove_dir_all(root).unwrap();
}

fn seed_installation(state: &Path, owns_body: bool) {
    let executable = state.join("installed-conduit");
    fs::write(&executable, b"reviewed installed product image").unwrap();
    let executable_sha = digest(&fs::read(&executable).unwrap());
    let body_state = if owns_body {
        let host_id = format!(
            "host/installed/{}",
            state.file_name().unwrap().to_string_lossy()
        );
        let body = Body::born(
            "source/installed-process-proof".into(),
            "checked/installed-process-proof".into(),
            1,
            SignId::from("sign/installed-process-proof/born"),
        )
        .unwrap();
        let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
        let mut biography = BodyBiographyEvidence::born(
            body.clone(),
            membership.clone(),
            "Independent installed process proof".into(),
        )
        .unwrap();
        let part = PartId::bind(&body.body_id, &host_id, 0).unwrap();
        let proof = MembershipProofId::bind("conduit/installed-process-proof/local-birth").unwrap();
        let admitted = membership
            .admit(
                &body.body_id,
                membership.revision,
                part.clone(),
                proof.clone(),
                SignId::from("sign/installed-process-proof/admitted"),
            )
            .unwrap();
        let present = membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: HostId::from(host_id.as_str()),
                    boot_id: BootId::from("boot/installed-process-proof/previous"),
                    offer_generation: OfferGeneration(1),
                    proof_id: proof,
                    sequence: 0,
                },
                SignId::from("sign/installed-process-proof/present"),
            )
            .unwrap();
        biography
            .append_membership_events(membership, &[(admitted, 2), (present, 3)])
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
        "durable host did not expose current runtime truth at {}",
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

fn assert_failure_contains(output: &Output, needle: &str, operation: &str) {
    assert!(
        !output.status.success(),
        "{operation} unexpectedly succeeded"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(needle),
        "{operation} did not report {needle:?}: {stderr}"
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
