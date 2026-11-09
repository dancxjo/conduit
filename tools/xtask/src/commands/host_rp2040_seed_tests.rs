use super::super::host_rp2040_membership_seed::{
    compose_retained_private_seed, PreparedMembershipBasis,
};
use super::*;
use conduit_host_make::{
    check_host_configuration, parse_host_configuration_conduit, CarrierTerminal,
};
use std::io::Cursor;
fn package_fixture(
    include_contract: bool,
    now: u64,
) -> (Vec<u8>, DeploymentRealizationReceipt, Pins) {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(include_str!(
            "../../../../targets/rp2040/profiles/pico-w-house-green.host.conduit"
        ))
        .unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let (release, spore, invitation, payloads) = fixture::fixture_with_output_metadata_at(
        &["host.uf2"],
        SporeOutputKind::Uf2,
        now,
        &checked,
        |image| {
            let mut metadata = serde_json::Map::new();
            if include_contract {
                metadata.insert(
                    conduit_host_rp2040::BOOTSTRAP_CONTRACT_METADATA.into(),
                    serde_json::to_value(conduit_host_rp2040::BootstrapContract::for_image(
                        &image.manifest.image_id,
                        checked.configuration_id(),
                    ))
                    .unwrap(),
                );
            }
            metadata
        },
    );
    let mut bytes = Vec::new();
    let receipt = conduit_body_make::write_joining_host_package(
        &release,
        &spore,
        &checked,
        &invitation,
        now + 1,
        |name| Ok(Cursor::new(payloads[name].as_slice())),
        &mut bytes,
        || false,
    )
    .unwrap();
    let exported = DeploymentRealizationReceipt {
        schema: "conduit.carrier/realization@1".into(),
        carrier_id: "fixture/download".into(),
        implementation_id: "conduit/download@1".into(),
        target_id: spore.manifest.target.clone(),
        spore_id: spore.manifest.spore_id.clone(),
        artifact_content_sha256: receipt.artifact_content_sha256,
        terminal: CarrierTerminal::Completed,
        bytes_realized: bytes.len() as u64,
        byte_verification_completed: true,
        explicit_authority_observed: false,
        boot_observed: false,
        join_observed: false,
        membership_admitted: false,
    };
    let pins = Pins {
        physical_uid: [1, 2, 3, 4, 5, 6, 7, 8],
        pinned_owner: FederationPeer {
            host_id: "fixture/owner".into(),
            boot_id: "fixture/owner/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
            verifying_key: [9; 32],
        },
    };
    (bytes, exported, pins)
}

#[test]
fn exported_package_composes_only_exact_private_bootstrap_and_public_basis() {
    let (bytes, exported, mut pins) = package_fixture(true, 0);
    let verified = verify_joining_host_package(
        &bytes,
        &exported,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        1,
    )
    .unwrap();
    let (bootstrap, basis) = compose(&verified, &pins).unwrap();
    assert_eq!(
        basis.expected_host.as_str(),
        "rp2040/flash/0102030405060708"
    );
    assert_eq!(
        basis.expected_profile.as_str(),
        verified.configuration().configuration_id()
    );
    assert_eq!(basis.invitation, verified.invitation().claim);
    assert_eq!(basis.spore_id, verified.spore().spore_id);
    let mut validator =
        conduit_rp2040_flash_layout::Uf2Validator::new(bootstrap.0.len() as u64).unwrap();
    for block in bootstrap.0.as_chunks::<512>().0 {
        validator.push(block).unwrap();
        let address = u32::from_le_bytes(block[12..16].try_into().unwrap());
        assert_eq!(
            conduit_rp2040_flash_layout::classify_image_page(address, 256).unwrap(),
            conduit_rp2040_flash_layout::ImageRegion::BodyBootstrap
        );
    }
    validator.finish().unwrap();
    pins.physical_uid = [0; 8];
    assert!(compose(&verified, &pins).is_err());
    pins.physical_uid = [1; 8];
    pins.pinned_owner.verifying_key = [0; 32];
    assert!(compose(&verified, &pins).is_err());
}

#[test]
fn checked_package_with_older_firmware_refuses_before_bootstrap_composition() {
    let (bytes, exported, pins) = package_fixture(false, 0);
    // Generic package integrity does not select a native bootstrap codec.
    let verified = verify_joining_host_package(
        &bytes,
        &exported,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        1,
    )
    .unwrap();
    let error = compose(&verified, &pins)
        .err()
        .expect("old firmware must refuse");
    assert_eq!(error, "reviewed firmware has no bootstrap codec contract");
}

#[test]
fn older_checked_package_creates_no_installation_directory() {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let (bytes, exported, pins) = package_fixture(false, at.as_millis().try_into().unwrap());
    let root = std::env::temp_dir().join(format!(
        "conduit-old-bootstrap-refusal-{}-{}",
        std::process::id(),
        at.as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let package = root.join("seed.zip");
    let receipt = root.join("export.json");
    let pin_file = root.join("pins.json");
    let output = root.join("prepared");
    publish(&package, &bytes).unwrap();
    publish(&receipt, &serde_json::to_vec(&exported).unwrap()).unwrap();
    let pins =
        serde_json::json!({"physical_uid":pins.physical_uid,"pinned_owner":pins.pinned_owner});
    publish(&pin_file, &serde_json::to_vec(&pins).unwrap()).unwrap();
    let result = prepare(&package, &receipt, Some(&pin_file), None, &output);
    let output_created = output.exists();
    fs::remove_dir_all(root).unwrap();
    assert_eq!(
        result.unwrap_err().to_string(),
        "reviewed firmware has no bootstrap codec contract"
    );
    assert!(!output_created);
}

#[test]
fn prepared_member_update_preserves_credential_and_key_and_refuses_foreign_body() {
    let (bytes, exported, mut pins) = package_fixture(true, 0);
    pins.pinned_owner.verifying_key = SpawnInvitationSecret::from_csprng_bytes([7; 32])
        .unwrap()
        .verifying_key();
    let verified = verify_joining_host_package(
        &bytes,
        &exported,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        1,
    )
    .unwrap();
    let root = std::env::temp_dir().join(format!(
        "retained-seed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let credential = serde_json::json!({
        "credential_id":"credential/retained", "body_id":verified.spore().body_id,
        "part_id":"part/retained", "host_id":"rp2040/flash/0102030405060708",
        "boot_id":"boot/previous", "issued_at_millis":1
    });
    let input = serde_json::json!({"physical_uid":pins.physical_uid,
        "pinned_owner":pins.pinned_owner,
        "admission":{"retained":{"credential":credential,"secret":([9u8;32])}}});
    let file = root.join("input.json");
    publish(&file, &serde_json::to_vec(&input).unwrap()).unwrap();
    let output = root.join("bootstrap.uf2");
    let basis = compose_retained_private_seed(
        verified.configuration(),
        &verified.image().manifest.image_id,
        &verified.spore().body_id,
        &file,
        &output,
    )
    .unwrap();
    let PreparedMembershipBasis::Retained(basis) = basis else {
        panic!("member became spawn")
    };
    assert_eq!(serde_json::to_value(&basis.credential).unwrap(), credential);
    assert_eq!(
        basis.member_verifying_key,
        SpawnInvitationSecret::from_csprng_bytes([9; 32])
            .unwrap()
            .verifying_key()
    );
    assert_eq!(basis.owner, pins.pinned_owner);
    let rejected = root.join("foreign.uf2");
    assert!(compose_retained_private_seed(
        verified.configuration(),
        &verified.image().manifest.image_id,
        "another/body",
        &file,
        &rejected,
    )
    .is_err());
    assert!(!rejected.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn application_closure_admits_only_exact_optional_calibrated_metadata() {
    let app = conduit_host_make::HostReleaseFile {
        path: "host.uf2".into(),
        bytes: 1,
        sha256: format!("sha256:{}", "a".repeat(64)),
        media_type: "application/x-uf2".into(),
    };
    let recipe = conduit_host_make::HostReleaseFile {
        path: "continuation-recipe.json".into(),
        bytes: 1,
        sha256: format!("sha256:{}", "b".repeat(64)),
        media_type: "application/json".into(),
    };
    assert!(application_files(std::slice::from_ref(&app)));
    assert!(application_files(&[app.clone(), recipe.clone()]));
    assert!(!application_files(&[recipe.clone(), app.clone()]));
    let mut foreign = recipe;
    foreign.path = "foreign.json".into();
    assert!(!application_files(&[app, foreign]));
}
