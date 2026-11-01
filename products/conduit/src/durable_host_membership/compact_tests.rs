use super::*;
use ed25519_dalek::{Signer, SigningKey};

fn invitation(root: [u8; 32]) -> Invitation {
    Invitation {
        invitation: PortableInvitation {
            schema: conduit_body::INVITATION_SCHEMA.into(),
            claim: serde_json::from_value(serde_json::json!({
                "invitation_id":"02".repeat(32), "body_id":"01".repeat(32),
                "nonce":vec![3;32], "expires_at_millis":300_000,
            }))
            .unwrap(),
            secret: [5; 32],
            rendezvous: None,
        },
        root_key: root,
        producer_host: "host/native-root".into(),
        producer_boot: "boot/native-root".into(),
    }
}
fn receipt(pending: &Pending, key: &SigningKey) -> PortableMembershipAdmissionReceipt {
    let mut receipt = PortableMembershipAdmissionReceipt {
        schema: conduit_body::MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA.into(),
        protocol_version: conduit_core::PROTOCOL_VERSION,
        credential: serde_json::from_value(serde_json::json!({
            "credential_id":"06".repeat(32), "body_id":pending.request.body_id,
            "part_id":"07".repeat(32), "host_id":pending.request.host_id,
            "boot_id":pending.request.boot_id, "issued_at_millis":100,
        }))
        .unwrap(),
        membership_revision: 2,
        request_digest: pending.request.digest(),
        producer_host: pending.invitation.producer_host.clone(),
        producer_boot: pending.invitation.producer_boot.clone(),
        producer_key: key.verifying_key().to_bytes(),
        invitation_id: pending.request.invitation_id.clone(),
        offer_generation: pending.request.offer_generation,
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
        signature: [0; 64],
    };
    receipt.signature = key.sign(&receipt.transcript()).to_bytes();
    receipt
}

#[test]
fn bounded_membership_keeps_real_identity_without_fabricating_equipment() {
    let host = conduit_std_host::StdHost::new();
    assert!(!host.advertisement().capabilities.is_empty());
    let key = SigningKey::from_bytes(&[7; 32]);
    let root = key.verifying_key().to_bytes();
    let pending = Pending::prepare(invitation(root), root, host.advertisement()).unwrap();
    pending.validate_current(host.advertisement()).unwrap();
    let encoded = serde_json::to_vec(&pending.request).unwrap();
    let decoded = PortableMembershipAdmissionRequest::decode(&encoded).unwrap();
    assert_eq!(decoded.host_id, host.advertisement().host_id);
    assert_eq!(decoded.boot_id, host.advertisement().boot_id);
    assert_eq!(
        decoded.offer_generation,
        host.advertisement().offer_generation
    );
    assert!(!std::str::from_utf8(&encoded)
        .unwrap()
        .contains("host_advertisement"));
    let signed = receipt(&pending, &key);
    pending.verify(&signed).unwrap();
    let mut wrong = host.advertisement().clone();
    wrong.boot_id = "boot/replaced".into();
    assert!(pending.validate_current(&wrong).is_err());
    wrong = host.advertisement().clone();
    wrong.offer_generation.0 += 1;
    assert!(pending.validate_current(&wrong).is_err());
    let mut wrong = signed;
    wrong.current_offers_available = true;
    wrong.signature = key.sign(&wrong.transcript()).to_bytes();
    assert!(pending.verify(&wrong).is_err());
    assert!(Pending::prepare(invitation(root), [9; 32], host.advertisement()).is_err());
}

#[test]
fn membership_commands_require_explicit_authorization_and_an_independent_pin() {
    use clap::Parser;
    for args in [
        vec![
            "conduit",
            "body",
            "accept-membership",
            "invitation.json",
            "--state-dir",
            "state",
            "--root-key",
            "pin",
        ],
        vec![
            "conduit",
            "body",
            "complete-membership",
            "receipt.json",
            "--state-dir",
            "state",
        ],
    ] {
        assert!(crate::cli::Cli::try_parse_from(args).is_err());
    }
    assert!(crate::cli::Cli::try_parse_from([
        "conduit",
        "body",
        "accept-membership",
        "invitation.json",
        "--state-dir",
        "state",
        "--authorize-join",
    ])
    .is_err());
    assert!(crate::cli::Cli::try_parse_from([
        "conduit",
        "body",
        "accept-membership",
        "invitation.json",
        "--state-dir",
        "state",
        "--root-key",
        "pin",
        "--authorize-join",
    ])
    .is_ok());
    assert!(crate::cli::Cli::try_parse_from([
        "conduit",
        "body",
        "complete-membership",
        "receipt.json",
        "--state-dir",
        "state",
        "--authorize-membership",
    ])
    .is_ok());
}

#[cfg(unix)]
#[test]
fn installed_membership_uses_authenticated_current_boot_and_verified_receipt() {
    use crate::durable_host as durable;
    let (manifest, state) = durable::tests::fixture();
    durable::install(&manifest, &state).unwrap();
    let (_, runtime) = durable::runtime_preparation::prepare_runtime(&state).unwrap();
    let actual = runtime.truth().advertisement;
    let key = SigningKey::from_bytes(&[7; 32]);
    let root = key.verifying_key().to_bytes();
    let input = state.join("membership-invitation.json");
    write_json_atomic(&input, &invitation(root)).unwrap();
    let (server, alias) =
        durable::invitation::current_offer_tests::serve_current(&state, Some(actual.clone()));
    let hex: String = root.iter().map(|b| format!("{b:02x}")).collect();
    accept(&input, &alias, &hex, true).unwrap();
    server.join().unwrap();
    fs::remove_file(alias).unwrap();
    fs::remove_file(state.join("control.sock")).unwrap();
    let pending: Pending =
        serde_json::from_slice(&fs::read(state.join("body/membership-join-private.json")).unwrap())
            .unwrap();
    assert_eq!(pending.request.host_id, actual.host_id);
    let output = state.join("membership-receipt.json");
    durable::write_bytes_atomic(
        &output,
        &serde_json::to_vec(&receipt(&pending, &key)).unwrap(),
    )
    .unwrap();
    let (server, alias) =
        durable::invitation::current_offer_tests::serve_current(&state, Some(actual.clone()));
    complete(&output, &alias, true).unwrap();
    server.join().unwrap();
    fs::remove_file(alias).unwrap();
    let installation = read_installation(&state.join("installation.json")).unwrap();
    let binding = installation.joined_body_state.as_ref().unwrap();
    assert_eq!(binding.body_id, pending.request.body_id.as_str());
    super::super::validate(binding, &installation).unwrap();
    let marker: RuntimeStatus =
        serde_json::from_slice(&fs::read(state.join("runtime.json")).unwrap()).unwrap();
    assert_eq!(marker.boot_id, pending.request.boot_id.as_str());
    assert_eq!(
        marker.body_id.as_deref(),
        Some(pending.request.body_id.as_str())
    );
    assert!(state.join("body/membership-join-private.json").exists());
    // Repeating completion after a lost local success indication reopens the
    // original receipt and credential; it does not create another admission.
    fs::remove_file(state.join("control.sock")).unwrap();
    let (server, alias) =
        durable::invitation::current_offer_tests::serve_current(&state, Some(actual));
    complete(&output, &alias, true).unwrap();
    server.join().unwrap();
    fs::remove_file(alias).unwrap();
    let unchanged = read_installation(&state.join("installation.json")).unwrap();
    assert_eq!(
        unchanged.joined_body_state.unwrap().credential_sha256,
        binding.credential_sha256
    );
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}
