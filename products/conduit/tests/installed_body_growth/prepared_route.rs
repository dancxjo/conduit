//! A real installed owner prepares transport before issuing the original invitation.
use super::*;

#[test]
fn prepared_route_carries_the_owners_next_original_invitation() {
    let root = unique_root();
    let owner = root.join("owner");
    let joining = root.join("joining");
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&joining).unwrap();
    seed_installation(&owner, true);
    seed_installation(&joining, false);
    let owner_service = start_host(&owner);
    let joining_service = start_host(&joining);
    wait_for_runtime(&owner);
    wait_for_runtime(&joining);

    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let certificate = root.join("owner-certificate.pem");
    let private_key = root.join("owner-private-key.pem");
    fs::write(&certificate, cert.pem()).unwrap();
    fs::write(&private_key, signing_key.serialize_pem()).unwrap();
    let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let bind = reservation.local_addr().unwrap().to_string();
    drop(reservation);
    let public_url = format!("wss://{bind}/body-admission");
    let args = [
        "body",
        "invite",
        "--prepare",
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
    ];
    let prepared = product(&args);
    assert_success(
        &prepared,
        "prepare actual TLS listener without issuing an invitation",
    );
    let candidate: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    assert!(candidate.get("claim").is_none());
    assert_eq!(candidate["reachability"], public_url);
    assert!(
        TcpListener::bind(&bind).is_err(),
        "prepared route owns the actual socket"
    );
    assert_failure_contains(
        &product(&args),
        "already live",
        "refuse replacing live route custody",
    );

    // This is the same ordinary invitation method used by Crèche. No second
    // invitation, modified descriptor or manually assembled admission phases.
    let issued = product(&[
        "body",
        "invite",
        "--state-dir",
        path(&owner),
        "--ttl-seconds",
        "30",
    ]);
    assert_success(&issued, "issue original invitation over prepared transport");
    let invitation: Value = serde_json::from_slice(&issued.stdout).unwrap();
    assert_eq!(invitation["schema"], "conduit.body/spawn-invitation@2");
    assert_eq!(invitation["rendezvous"]["candidates"], json!([candidate]));
    assert_eq!(
        invitation["rendezvous"]["invitation_id"],
        invitation["claim"]["invitation_id"]
    );
    let invitation_path = root.join("original-invitation.json");
    fs::write(&invitation_path, &issued.stdout).unwrap();
    let joined = product(&[
        "body",
        "join",
        path(&invitation_path),
        "--state-dir",
        path(&joining),
        "--authorize-join",
    ]);
    assert_success(
        &joined,
        "join through the exact original prepared invitation",
    );
    let status = product(&["body", "status", "--state-dir", path(&joining), "--json"]);
    assert_success(&status, "inspect retained admitted membership");
    let status: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["body_id"], invitation["claim"]["body_id"]);
    assert_eq!(status["membership"], "admitted");
    assert_eq!(status["plan_created"], false);
    assert_eq!(status["play_created"], false);
    drop(joining_service);
    drop(owner_service);
    fs::remove_dir_all(root).unwrap();
}
