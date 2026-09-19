use super::*;

#[test]
fn provisioning_creates_distinct_private_exact_descriptors_without_overwrite() {
    let output = std::env::temp_dir().join(format!(
        "conduit-relay-provision-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let options = || ProvisionOptions {
        relay_address: "192.0.2.10:7443".into(),
        relay_url: "wss://relay.example:7443/conduit".into(),
        server_identity: "relay.example".into(),
        certificate_sha256: "11".repeat(32),
        first_host_id: "host/first".into(),
        first_boot_id: "boot/first".into(),
        second_host_id: "host/second".into(),
        second_boot_id: "boot/second".into(),
        output: output.clone(),
        expires_in_seconds: 60,
        maximum_attempts: 1,
        authorize_provision: true,
    };
    provision(options()).unwrap();
    let slot: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("relay-slot.json")).unwrap()).unwrap();
    let first: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("endpoint-first.json")).unwrap()).unwrap();
    let second: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("endpoint-second.json")).unwrap()).unwrap();
    assert_eq!(slot["schema"], SLOT_SCHEMA);
    assert_eq!(first["candidate"]["route_id"], slot["route_id"]);
    assert_eq!(second["candidate"]["route_id"], slot["route_id"]);
    assert_eq!(first["candidate"]["role"], "initiator");
    assert_eq!(second["candidate"]["role"], "responder");
    assert_ne!(
        first["candidate"]["relay_capability"],
        second["candidate"]["relay_capability"]
    );
    assert_eq!(
        first["candidate"]["protected_session_psk"],
        second["candidate"]["protected_session_psk"]
    );
    let now = now_millis().unwrap();
    crate::host_rendezvous::validate_relay_endpoint_descriptor(
        &output.join("endpoint-first.json"),
        now,
    )
    .unwrap();
    crate::host_rendezvous::validate_relay_endpoint_descriptor(
        &output.join("endpoint-second.json"),
        now,
    )
    .unwrap();
    assert!(provision(options()).unwrap_err().contains("already exists"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(output.join("relay-slot.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    fs::remove_dir_all(output).unwrap();
}

#[test]
fn provisioning_refuses_missing_authority_and_non_ascii_certificate_digest() {
    let mut options = ProvisionOptions {
        relay_address: "192.0.2.10:7443".into(),
        relay_url: "wss://relay.example:7443/conduit".into(),
        server_identity: "relay.example".into(),
        certificate_sha256: "11".repeat(32),
        first_host_id: "host/first".into(),
        first_boot_id: "boot/first".into(),
        second_host_id: "host/second".into(),
        second_boot_id: "boot/second".into(),
        output: std::env::temp_dir().join("conduit-relay-provision-must-not-exist"),
        expires_in_seconds: 60,
        maximum_attempts: 1,
        authorize_provision: false,
    };
    assert!(provision(options).unwrap_err().contains("requires"));

    options = ProvisionOptions {
        relay_address: "192.0.2.10:7443".into(),
        relay_url: "wss://relay.example:7443/conduit".into(),
        server_identity: "relay.example".into(),
        certificate_sha256: "é".repeat(32),
        first_host_id: "host/first".into(),
        first_boot_id: "boot/first".into(),
        second_host_id: "host/second".into(),
        second_boot_id: "boot/second".into(),
        output: std::env::temp_dir().join("conduit-relay-provision-must-not-exist"),
        expires_in_seconds: 60,
        maximum_attempts: 1,
        authorize_provision: true,
    };
    assert!(provision(options).unwrap_err().contains("64 hex digits"));
}
