use super::*;
use crate::BrowserMakePackage;
use conduit_host_make::{
    build_host_image, check_host_configuration, parse_host_configuration_conduit,
    verify_host_release_bytes, BuildInputs, HostReleaseBounds, MakeCatalog, MakePackageSet,
    ReleaseTargetProfile,
};
use std::io::Cursor;
const FIXTURE: &str = include_str!("fixtures/js-browser-bundle.json");
const CONFIGURATION: &str =
    include_str!("../../../../../targets/browser/profiles/environment-recorder.host.conduit");

fn fixture() -> serde_json::Value {
    serde_json::from_str(FIXTURE).unwrap()
}
fn checked(source: &str) -> CheckedHostConfiguration {
    let packages = MakePackageSet::compose(&[&BrowserMakePackage]).unwrap();
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap()
}
fn generic(checked: &CheckedHostConfiguration) -> HostImage {
    let packages = MakePackageSet::compose(&[&BrowserMakePackage]).unwrap();
    build_host_image(
        checked.profile().clone(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
        &SporeOutputKind::BrowserBundle,
        &BuildInputs {
            source_identity: "source/js-differential-fixture".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
}
fn byteproof(value: &serde_json::Value, key: &str) -> VerifiedHostReleaseBytes {
    let manifest = &value[key];
    let payloads = value["payloads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["path"].as_str().unwrap(),
                p["bytes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_u64().unwrap() as u8)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    verify_host_release_bytes(
        &serde_json::to_vec(manifest).unwrap(),
        &ReleaseTargetProfile {
            target_id: TARGET,
            package_id: "browser-wasm@1",
            output: "browser-bundle",
            builder_adapter: manifest["builder_adapter"].as_str().unwrap(),
            deployment_adapter: "conduit-host-browser/load@1",
        },
        "source/js-differential-fixture",
        HostReleaseBounds {
            maximum_manifest_bytes: 65536,
            maximum_files: 16,
            maximum_file_bytes: 1024 * 1024,
            maximum_bundle_bytes: 1024 * 1024,
        },
        |path| {
            payloads
                .get(path)
                .cloned()
                .map(Cursor::new)
                .ok_or("fixture payload absent".into())
        },
        || false,
    )
    .unwrap()
}
fn image_bytes(value: &serde_json::Value) -> Vec<u8> {
    value["payloads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["path"] == IMAGE)
        .unwrap()["bytes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_u64().unwrap() as u8)
        .collect()
}
fn verify(
    value: &serde_json::Value,
    configuration: &CheckedHostConfiguration,
) -> Result<VerifiedBrowserBundleImage, String> {
    verify_browser_bundle_image(
        configuration,
        &generic(configuration),
        byteproof(value, "release"),
        byteproof(value, "distribution"),
        &image_bytes(value),
    )
}

#[test]
fn rust_matches_exact_existing_js_build_basis_and_image_fixture() {
    let value = fixture();
    let configuration = checked(CONFIGURATION);
    assert_eq!(
        configuration.configuration_id(),
        value["checked"]["configuration_id"].as_str().unwrap()
    );
    let proof = verify(&value, &configuration).unwrap();
    assert_eq!(
        proof.bundle_image_id(),
        "image:sha256:cd6d9ba70f90eeb92f48c7e415eb06dbebcf89a0d5cd501c0a6876af116a6327"
    );
    assert_eq!(
        proof.image_content_digest(),
        value["release"]["bundle_sha256"].as_str().unwrap()
    );
    assert_ne!(proof.generic_image_id(), proof.bundle_image_id());
    assert_eq!(
        proof.profile_id(),
        value["checked"]["profile_id"].as_str().unwrap()
    );
    assert_eq!(proof.configuration_id(), configuration.configuration_id());
}

#[test]
fn target_proof_refuses_another_checked_host_even_with_valid_payload_hashes() {
    let value = fixture();
    let other =
        checked(&CONFIGURATION.replace("host environment-recorder", "host another-browser"));
    assert!(verify(&value, &other).is_err());
}

#[test]
fn generic_byteproof_does_not_authorize_forged_rich_browser_metadata() {
    for field in [
        "browser_image_id",
        "browser_build_id",
        "browser_configuration_id",
        "browser_profile_id",
        "distribution_sha256",
    ] {
        let mut value = fixture();
        value["release"][field] = format!("sha256:{}", "0".repeat(64)).into();
        // The bytes proof properly succeeds; only the target checker can refuse
        // these semantic claims carried outside the payload aggregate.
        assert!(verify(&value, &checked(CONFIGURATION)).is_err());
    }
}

#[test]
fn target_proof_refuses_module_cycles_absent_providers_and_wrong_revisions() {
    for variant in 0..3 {
        let mut value = fixture();
        let reviewed = &mut value["distribution"]["reviewed_distribution"];
        match variant {
            0 => reviewed["modules"][0]["dependencies"] = serde_json::json!([BOOT]),
            1 => {
                reviewed["implementations"].as_array_mut().unwrap().pop();
            }
            _ => reviewed["implementations"][0]["revision"] = 2.into(),
        }
        assert!(verify(&value, &checked(CONFIGURATION)).is_err());
    }
}

#[test]
fn target_proof_refuses_supplied_image_bytes_outside_verified_release() {
    let value = fixture();
    let configuration = checked(CONFIGURATION);
    let mut image = image_bytes(&value);
    image[0] = b' ';
    assert!(verify_browser_bundle_image(
        &configuration,
        &generic(&configuration),
        byteproof(&value, "release"),
        byteproof(&value, "distribution"),
        &image
    )
    .is_err());
}

#[test]
fn reordered_rich_image_keys_refuse_even_when_every_payload_hash_is_resealed() {
    let mut value = fixture();
    let semantic: serde_json::Value = serde_json::from_slice(&image_bytes(&value)).unwrap();
    // Value's map serialization sorts keys, unlike the existing JS producer.
    let reordered = serde_json::to_vec(&semantic).unwrap();
    let digest = sha(&reordered);
    let payload = value["payloads"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["path"] == IMAGE)
        .unwrap();
    payload["bytes"] = serde_json::to_value(&reordered).unwrap();
    let file = value["release"]["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["path"] == IMAGE)
        .unwrap();
    file["bytes"] = (reordered.len() as u64).into();
    file["sha256"] = digest.into();
    let mut aggregate = "conduit.release/host-bundle-content@1\0".to_owned();
    for file in value["release"]["files"].as_array().unwrap() {
        aggregate.push_str(file["path"].as_str().unwrap());
        aggregate.push('\0');
        aggregate.push_str(file["sha256"].as_str().unwrap());
        aggregate.push('\n');
    }
    value["release"]["bundle_sha256"] = sha(aggregate.as_bytes()).into();
    assert!(verify(&value, &checked(CONFIGURATION))
        .err()
        .unwrap()
        .contains("field order"));
}

#[test]
#[ignore = "requires an explicitly selected retained real prior-Make browser artifact directory"]
fn retained_real_browser_bundle_matches_checked_source_and_generic_make_image() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_REVIEWED_BROWSER_FIXTURE_ROOT")
            .expect("select the authorized artifact directory"),
    );
    let host_image: HostImage =
        serde_json::from_slice(&std::fs::read(root.join("checked-image/image.json")).unwrap())
            .unwrap();
    let configuration = checked(CONFIGURATION);
    let open_release = |directory: &str, manifest: &str, builder: &str| {
        let directory = root.join(directory);
        let bytes = std::fs::read(directory.join(manifest)).unwrap();
        verify_host_release_bytes(
            &bytes,
            &ReleaseTargetProfile {
                target_id: TARGET,
                package_id: "browser-wasm@1",
                output: "browser-bundle",
                builder_adapter: builder,
                deployment_adapter: "conduit-host-browser/load@1",
            },
            &host_image.manifest.source_identity,
            HostReleaseBounds {
                maximum_manifest_bytes: 256 * 1024,
                maximum_files: 16,
                maximum_file_bytes: 20 * 1024 * 1024,
                maximum_bundle_bytes: 48 * 1024 * 1024,
            },
            |path| std::fs::File::open(directory.join(path)).map_err(|e| e.to_string()),
            || false,
        )
        .unwrap()
    };
    let release = open_release(
        "bundle",
        "browser-bundle-release.json",
        "conduit-host-browser/bind-prebuilt@1",
    );
    let distribution = open_release(
        "distribution",
        "browser-page.json",
        "conduit-host-browser/build-wasm@1",
    );
    let bytes = std::fs::read(root.join("bundle/conduit-browser-image.json")).unwrap();
    let proof =
        verify_browser_bundle_image(&configuration, &host_image, release, distribution, &bytes)
            .unwrap();
    println!(
        "generic={} browser={} profile={} aggregate={} bytes={}",
        proof.generic_image_id(),
        proof.bundle_image_id(),
        proof.profile_id(),
        proof.image_content_digest(),
        proof.release().payload_bytes()
    );
}
