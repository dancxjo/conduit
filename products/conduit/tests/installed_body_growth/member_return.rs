//! The ordinary CLI uses authenticated actor control while the service owns its locks.
use super::*;
use std::io::{BufRead, BufReader};

#[test]
fn explicit_member_return_uses_live_service_boot_and_preserves_body_and_part() {
    let root = unique_root();
    let owner = root.join("owner");
    let joining = root.join("joining");
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&joining).unwrap();
    seed_installation(&owner, true);
    seed_installation(&joining, false);
    let _owner = start_host(&owner);
    let _joining = start_host(&joining);
    wait_for_runtime(&owner);
    wait_for_runtime(&joining);
    let invitation = product(&["body", "invite", "--state-dir", path(&owner)]);
    assert_success(&invitation, "issue invitation");
    let request = product_with_stdin(
        &[
            "body",
            "accept",
            "-",
            "--state-dir",
            path(&joining),
            "--authorize-join",
        ],
        &invitation.stdout,
    );
    assert_success(&request, "accept invitation");
    let admitted = product_with_stdin(
        &[
            "body",
            "admit",
            "-",
            "--state-dir",
            path(&owner),
            "--authorize-admission",
        ],
        &request.stdout,
    );
    assert_success(&admitted, "admit member");
    let receipt = root.join("receipt.json");
    fs::write(&receipt, &admitted.stdout).unwrap();
    assert_success(
        &product(&[
            "body",
            "complete-join",
            path(&receipt),
            "--state-dir",
            path(&joining),
            "--authorize-membership",
        ]),
        "retain membership",
    );
    let credential = joining.join("body/membership-credential.json");
    let prior: Value = serde_json::from_slice(&fs::read(&credential).unwrap()).unwrap();
    assert_success(
        &product(&[
            "body",
            "member-leave",
            "--state-dir",
            path(&owner),
            "--member-credential",
            path(&credential),
            "--authorize-leave",
        ]),
        "detach existing member",
    );
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let cert_path = root.join("cert.pem");
    let key_path = root.join("key.pem");
    fs::write(&cert_path, cert.pem()).unwrap();
    fs::write(&key_path, signing_key.serialize_pem()).unwrap();
    let reserved = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let bind = reserved.local_addr().unwrap();
    drop(reserved);
    let url = format!("wss://127.0.0.1:{}/member", bind.port());
    let mut route = RunningHost(
        Command::new(env!("CARGO_BIN_EXE_conduit"))
            .args([
                "body",
                "member-route",
                "--state-dir",
                path(&owner),
                "--member-credential",
                path(&credential),
                "--ttl-seconds",
                "30",
                "--route-bind",
                &bind.to_string(),
                "--route-url",
                &url,
                "--route-tls-cert",
                path(&cert_path),
                "--route-tls-key",
                path(&key_path),
                "--authorize-route",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(route.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let descriptor = root.join("member-route.json");
    fs::write(&descriptor, line).unwrap();
    let refused = product(&[
        "body",
        "return",
        "--state-dir",
        path(&joining),
        "--descriptor",
        path(&descriptor),
    ]);
    assert_failure_contains(&refused, "--authorize-return", "unauthorized return");
    let returned = product(&[
        "body",
        "return",
        "--state-dir",
        path(&joining),
        "--descriptor",
        path(&descriptor),
        "--authorize-return",
    ]);
    assert_success(&returned, "return through locked service actor");
    let next: Value = serde_json::from_slice(&returned.stdout).unwrap();
    for field in ["body_id", "part_id", "host_id", "boot_id"] {
        assert_eq!(next[field], prior[field]);
    }
    assert_ne!(next["credential_id"], prior["credential_id"]);
    // Same running Host Boot, new exact returned credential, no new invitation or Play.
    let status = product(&["body", "status", "--state-dir", path(&joining), "--json"]);
    assert_success(&status, "inspect returned member");
    let status: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["membership_admitted"], true);
    assert_eq!(status["play_created"], false);
    drop(route);
    fs::remove_dir_all(root).unwrap();
}
