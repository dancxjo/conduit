use super::*;

pub(crate) fn fixture() -> (Vec<u8>, BodyBoundArtifactIdentity) {
    let checked = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(include_str!(
            "../../../targets/std/profiles/linux-computer.host.conduit"
        ))
        .unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let source =
        conduit_host_make::canonical_host_configuration_conduit(checked.configuration()).unwrap();
    let profile = conduit_host_make::validate_profile(
        checked.profile().clone(),
        &conduit_workspace_make::catalog(),
    )
    .unwrap();
    let (image, _) = conduit_host_make::build_host_image(
        checked.profile().clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &conduit_host_make::SporeOutputKind::NativeBundle,
        &conduit_host_make::BuildInputs {
            source_identity: "commit:reviewed".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    let payloads = vec![
        ZipEntry {
            name: "conduit-linux-x86_64".into(),
            bytes: b"reviewed product".to_vec(),
        },
        ZipEntry {
            name: "install-linux-x86_64.sh".into(),
            bytes: b"#!/bin/sh\nexit 0\n".to_vec(),
        },
    ];
    let files = payloads
        .iter()
        .map(|entry| ReleaseFile {
            path: entry.name.clone(),
            bytes: entry.bytes.len() as u64,
            sha256: digest(&entry.bytes),
        })
        .collect::<Vec<_>>();
    let image_content_sha256 = bundle_digest(&files);
    let release_manifest = serde_json::json!({
        "schema":"conduit.release/host-bundle@1",
        "target_id":"std/x86_64/computer",
        "make_package_id":"hosted-native@1",
        "output":"native-bundle",
        "builder_adapter":"conduit-host-hosted/build-native@1",
        "deployment_adapter":"conduit-host-hosted/launch@1",
        "source_identity":"commit:reviewed",
        "bundle_sha256":image_content_sha256,
        "files":files.iter().map(|file|serde_json::json!({"path":file.path,"bytes":file.bytes,"sha256":file.sha256,"media_type":"application/octet-stream"})).collect::<Vec<_>>(),
        "retained_target_metadata":{"example":true}
    });
    let provision = serde_json::to_vec(&serde_json::json!({
        "schema": "conduit.spore/native-package-provision@1",
        "release_manifest": release_manifest,
        "host_configuration_source": source,
        "host_image": image,
        "spore": {
            "schema": "conduit.body/spore-manifest@2",
            "spore_id": "spore/body-one/native-host",
            "body_id": "body/one",
            "binding": {"mode":"self-joining", "invitation_id":"invitation/one"},
            "body_description_id": "body-description/one",
            "host_entry_name": "native-host",
            "host_configuration_id": checked.configuration_id(),
            "profile_id": profile.profile_id().as_str(),
            "build_id": image.manifest.build_id,
            "image_id": image.manifest.image_id,
            "image_content_digest": image_content_sha256,
            "target": "std/x86_64/computer",
            "output": "native-bundle",
            "make": {
                "make_package_id": "hosted-native@1",
                "make_package_revision": 1,
                "toolchain_identity": "rust/reviewed",
                "builder_adapter": "conduit-host-hosted/build-native@1",
                "strategy": "deterministic-specialized-build",
                "deployment_adapter": "conduit-host-hosted/launch@1",
                "post_build_actions": [],
                "output": "native-bundle",
                "maxima": {"maximum_hosts":1},
                "features": [],
                "selected_base_implementations": [],
                "implementation_packages": []
            },
            "source_identity": "sign/original-body-birth"
        },
        "portable_invitation": {
            "schema": "conduit.body/spawn-invitation@1",
            "claim": { "invitation_id": "invitation/one", "body_id": "body/one", "nonce": vec![7_u8; 32], "expires_at_millis": 4_000_000_000_000_u64 },
            "secret": vec![9_u8; 32]
        },
        "invitation_provision": {
            "invitation_id": "invitation/one",
            "nonce": vec![7_u8; 32],
            "expires_at_millis": 4_000_000_000_000_u64,
            "secret": vec![9_u8; 32],
            "rendezvous_candidates": []
        }
    }))
    .unwrap();
    let mut entries = payloads;
    entries.push(ZipEntry {
        name: PROVISION_PATH.into(),
        bytes: provision,
    });
    let package = stored_zip(&entries);
    let artifact = BodyBoundArtifactIdentity {
        target_id: "std/x86_64/computer".into(),
        image_id: image.manifest.image_id.clone(),
        image_content_sha256,
        spore_id: "spore/body-one/native-host".into(),
        artifact_content_sha256: digest(&package),
        artifact_bytes: package.len() as u64,
    };
    (package, artifact)
}

#[test]
fn exact_body_bound_zip_becomes_a_verified_release_without_persisting_the_secret() {
    let (package, artifact) = fixture();
    let root = test_root("exact");
    fs::create_dir_all(&root).unwrap();
    let source = root.join("spore.zip");
    let state = root.join("state");
    fs::write(&source, &package).unwrap();

    let prepared = prepare(&source, &artifact, &state).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&prepared.manifest).unwrap()).unwrap();
    assert_eq!(manifest["target_id"], artifact.target_id);
    assert_eq!(manifest["source_identity"], "commit:reviewed");
    assert_eq!(manifest["retained_target_metadata"]["example"], true);
    assert_eq!(manifest["bundle_sha256"], artifact.image_content_sha256);
    assert_eq!(manifest["files"].as_array().unwrap().len(), 2);
    assert!(prepared.root.join("conduit-linux-x86_64").is_file());
    assert!(!prepared.root.join(PROVISION_PATH).exists());
    assert!(!fs::read_dir(&prepared.root).unwrap().any(|entry| {
        fs::read(entry.unwrap().path())
            .unwrap()
            .windows(32)
            .any(|window| window == [9_u8; 32])
    }));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_artifact_and_mismatched_image_refuse_before_staging() {
    let (package, mut artifact) = fixture();
    let root = test_root("refusal");
    fs::create_dir_all(&root).unwrap();
    let source = root.join("spore.zip");
    let state = root.join("state");
    fs::write(&source, &package).unwrap();

    artifact.image_id = "image/other".into();
    assert_eq!(
        prepare(&source, &artifact, &state).unwrap_err(),
        NativeInstallRefusal::BindingMismatch
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);

    artifact.image_id = "image/native-reviewed".into();
    fs::write(&source, [package.as_slice(), b"tamper"].concat()).unwrap();
    assert_eq!(
        prepare(&source, &artifact, &state).unwrap_err(),
        NativeInstallRefusal::ContentMismatch
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

fn stored_zip(entries: &[ZipEntry]) -> Vec<u8> {
    let mut locals = Vec::new();
    let mut centrals = Vec::new();
    let mut local_offset = 0_u32;
    for entry in entries {
        let name = entry.name.as_bytes();
        let crc = crc32(&entry.bytes);
        let mut local = vec![0_u8; 30 + name.len() + entry.bytes.len()];
        put_u32(&mut local, 0, LOCAL_FILE);
        put_u16(&mut local, 4, 20);
        put_u16(&mut local, 6, 0x0800);
        put_u32(&mut local, 14, crc);
        put_u32(&mut local, 18, entry.bytes.len() as u32);
        put_u32(&mut local, 22, entry.bytes.len() as u32);
        put_u16(&mut local, 26, name.len() as u16);
        local[30..30 + name.len()].copy_from_slice(name);
        local[30 + name.len()..].copy_from_slice(&entry.bytes);

        let mut central = vec![0_u8; 46 + name.len()];
        put_u32(&mut central, 0, CENTRAL_FILE);
        put_u16(&mut central, 4, 0x0314);
        put_u16(&mut central, 6, 20);
        put_u16(&mut central, 8, 0x0800);
        put_u32(&mut central, 16, crc);
        put_u32(&mut central, 20, entry.bytes.len() as u32);
        put_u32(&mut central, 24, entry.bytes.len() as u32);
        put_u16(&mut central, 28, name.len() as u16);
        put_u32(&mut central, 42, local_offset);
        central[46..].copy_from_slice(name);
        local_offset += local.len() as u32;
        locals.extend(local);
        centrals.extend(central);
    }
    let central_bytes = centrals.len() as u32;
    let mut end = vec![0_u8; 22];
    put_u32(&mut end, 0, END);
    put_u16(&mut end, 8, entries.len() as u16);
    put_u16(&mut end, 10, entries.len() as u16);
    put_u32(&mut end, 12, central_bytes);
    put_u32(&mut end, 16, local_offset);
    locals.extend(centrals);
    locals.extend(end);
    locals
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn test_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "conduit-native-package-{name}-{}",
        std::process::id()
    ))
}

#[path = "native_package_install/tests/joining_host_package.rs"]
mod joining_host_package;

#[test]
fn missing_or_inconsistent_make_provenance_is_refused_before_staging() {
    for (label, change) in [
        ("missing", 0),
        ("foreign-target", 1),
        ("wrong-file-hash", 2),
        ("missing-file", 3),
        ("wrong-aggregate", 4),
    ] {
        let (original, mut artifact) = fixture();
        let mut entries = read_stored_zip(&original).unwrap();
        let provision = entries
            .iter_mut()
            .find(|entry| entry.name == PROVISION_PATH)
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&provision.bytes).unwrap();
        match change {
            0 => {
                value.as_object_mut().unwrap().remove("release_manifest");
            }
            1 => value["release_manifest"]["target_id"] = "std/foreign/computer".into(),
            2 => {
                value["release_manifest"]["files"][0]["sha256"] =
                    format!("sha256:{}", "2".repeat(64)).into()
            }
            3 => {
                value["release_manifest"]["files"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            _ => {
                value["release_manifest"]["bundle_sha256"] =
                    format!("sha256:{}", "3".repeat(64)).into()
            }
        }
        provision.bytes = serde_json::to_vec(&value).unwrap();
        let package = stored_zip(&entries);
        // Even when supplied as a newly selected artifact, inconsistent Make
        // metadata cannot pass the existing byte-inventory verifier.
        artifact.artifact_bytes = package.len() as u64;
        artifact.artifact_content_sha256 = digest(&package);
        let root = test_root(label);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("spore.zip");
        fs::write(&source, package).unwrap();
        let result = prepare(&source, &artifact, &root.join("state"));
        assert!(matches!(
            result,
            Err(NativeInstallRefusal::BindingMismatch | NativeInstallRefusal::ImageMismatch)
        ));
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn artifact_digest_authenticates_make_source_metadata() {
    let (original, artifact) = fixture();
    let mut entries = read_stored_zip(&original).unwrap();
    let provision = entries
        .iter_mut()
        .find(|entry| entry.name == PROVISION_PATH)
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&provision.bytes).unwrap();
    value["release_manifest"]["source_identity"] = "source:tampered".into();
    provision.bytes = serde_json::to_vec(&value).unwrap();
    let root = test_root("source-tamper");
    fs::create_dir_all(&root).unwrap();
    let source = root.join("spore.zip");
    let tampered = stored_zip(&entries);
    assert_eq!(tampered.len() as u64, artifact.artifact_bytes);
    fs::write(&source, tampered).unwrap();
    assert!(matches!(
        prepare(&source, &artifact, &root.join("state")),
        Err(NativeInstallRefusal::ContentMismatch)
    ));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn original_seed_invitation_cannot_be_missing_or_diverge_from_zip_binding() {
    for change in 0..3 {
        let (package, mut artifact) = fixture();
        let mut entries = read_stored_zip(&package).unwrap();
        let entry = entries
            .iter_mut()
            .find(|e| e.name == PROVISION_PATH)
            .unwrap();
        let mut provision: serde_json::Value = serde_json::from_slice(&entry.bytes).unwrap();
        match change {
            0 => {
                provision
                    .as_object_mut()
                    .unwrap()
                    .remove("portable_invitation");
            }
            1 => provision["portable_invitation"]["claim"]["body_id"] = "body/foreign".into(),
            _ => provision["portable_invitation"]["secret"] = serde_json::json!(vec![91; 32]),
        }
        entry.bytes = serde_json::to_vec(&provision).unwrap();
        let package = stored_zip(&entries);
        artifact.artifact_bytes = package.len() as u64;
        artifact.artifact_content_sha256 = digest(&package);
        let root = test_root(&format!("original-invitation-{change}"));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("seed.zip");
        fs::write(&source, package).unwrap();
        assert!(matches!(
            prepare(&source, &artifact, &root.join("state")),
            Err(NativeInstallRefusal::BindingMismatch)
        ));
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn missing_or_divergent_selected_source_and_image_refuse_before_installation() {
    for change in 0..6 {
        let (package, mut artifact) = fixture();
        let mut entries = read_stored_zip(&package).unwrap();
        let entry = entries
            .iter_mut()
            .find(|e| e.name == PROVISION_PATH)
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&entry.bytes).unwrap();
        match change {
            0 => {
                value
                    .as_object_mut()
                    .unwrap()
                    .remove("host_configuration_source");
            }
            1 => {
                value.as_object_mut().unwrap().remove("host_image");
            }
            2 => {
                value["host_configuration_source"] = value["host_configuration_source"]
                    .as_str()
                    .unwrap()
                    .replace("linux-computer", "other-computer")
                    .into()
            }
            3 => value["spore"]["profile_id"] = "profile/foreign".into(),
            4 => value["host_image"]["manifest"]["bounds"]["timer_slots"] = 999999.into(),
            _ => value["host_image"]["manifest"]["source_identity"] = "source/foreign".into(),
        }
        entry.bytes = serde_json::to_vec(&value).unwrap();
        let package = stored_zip(&entries);
        artifact.artifact_bytes = package.len() as u64;
        artifact.artifact_content_sha256 = digest(&package);
        let root = test_root(&format!("selected-host-custody-{change}"));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("seed.zip");
        fs::write(&source, package).unwrap();
        assert!(matches!(
            prepare(&source, &artifact, &root.join("state")),
            Err(NativeInstallRefusal::BindingMismatch)
        ));
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
