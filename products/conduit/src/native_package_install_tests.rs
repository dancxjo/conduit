use super::*;

fn fixture() -> (Vec<u8>, BodyBoundArtifactIdentity) {
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
    let provision = serde_json::to_vec(&serde_json::json!({
        "schema": "conduit.spore/native-package-provision@1",
        "spore": {
            "schema": "conduit.body/spore-manifest@2",
            "spore_id": "spore/body-one/native-host",
            "body_id": "body/one",
            "binding": {"mode":"self-joining", "invitation_id":"invitation/one"},
            "body_description_id": "body-description/one",
            "host_entry_name": "native-host",
            "host_configuration_id": "host-configuration/native",
            "profile_id": "host-profile/native",
            "build_id": "build/native",
            "image_id": "image/native-reviewed",
            "image_content_digest": image_content_sha256,
            "target": "std/x86_64/computer",
            "output": "native-bundle",
            "fabrication": {
                "fabrication_package_id": "hosted-native@1",
                "fabrication_package_revision": 1,
                "toolchain_identity": "rust/reviewed",
                "builder_adapter": "conduit-host-hosted/build-native@1",
                "deployment_adapter": "conduit-host-hosted/launch@1",
                "post_build_actions": [],
                "output": "native-bundle",
                "maxima": {"maximum_hosts":1},
                "features": [],
                "selected_base_implementations": [],
                "implementation_packages": []
            },
            "source_identity": "commit:reviewed"
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
        image_id: "image/native-reviewed".into(),
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
