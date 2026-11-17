use super::*;
use conduit_host_make::{HostReleaseFile, HostReleaseManifest};
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
pub(crate) fn fixture() -> (HostReleaseManifest, CheckedHostConfiguration) {
    let ids = [
        "browser/dom@1",
        "browser/keyboard-events@1",
        "browser/pointer-events@1",
    ];
    let configuration =
        crate::creche::check_womb_configuration(crate::creche::BrowserConfigurationSelection {
            catalog_generation: 1,
            implementations: ids.iter().map(|id| (*id).into()).collect(),
        })
        .unwrap();
    let files = vec![
        HostReleaseFile {
            path: "runtime.wasm".into(),
            bytes: 8,
            sha256: digest(b"\0asm\x01\0\0\0"),
            media_type: "application/wasm".into(),
        },
        HostReleaseFile {
            path: "browser-boot-profile.mjs".into(),
            bytes: 8,
            sha256: digest(b"export{}"),
            media_type: "text/javascript".into(),
        },
    ];
    let mut aggregate = Sha256::new();
    aggregate.update(b"conduit.release/host-bundle-content@1\0");
    for file in &files {
        aggregate.update(file.path.as_bytes());
        aggregate.update(b"\0");
        aggregate.update(file.sha256.as_bytes());
        aggregate.update(b"\n");
    }
    let manifest=serde_json::from_value(serde_json::json!({
        "schema":"conduit.release/host-bundle@1","target_id":"browser/wasm32/page","make_package_id":"browser-wasm@1","output":"browser-bundle",
        "builder_adapter":"conduit-host-browser/build-wasm@1","deployment_adapter":"conduit-host-browser/load@1","source_identity":"source/conformance",
        "bundle_sha256":format!("sha256:{:x}",aggregate.finalize()),"files":files,
        "reviewed_distribution":{"schema":"conduit.browser/reviewed-distribution@1","distribution_id":"conduit.browser/reviewed-distribution@1",
            "runtime_abi":"conduit.browser/runtime-abi@1","toolchain_identity":"toolchain/conformance","source_commit":"source/conformance",
            "maximum_bundle_bytes":67108864,"targets":["browser/wasm32/page"],"implementations":ids.iter().map(|id|serde_json::json!({"id":id,"revision":1,"artifact":"browser-runtime-superset.wasm"})).collect::<Vec<_>>(),
            "modules":[{"path":"browser-boot-profile.mjs","dependencies":[]}]}
    })).unwrap();
    (manifest, configuration)
}

// Minimal IMAGE bytes emitted by the existing JS buildBrowserBundleImage,
// consumed by the private Host-completion ABI proof; no runtime producer here.
pub(crate) fn output() -> (Vec<u8>, Vec<u8>) {
    let (mut manifest, _) = fixture();
    let image = include_bytes!("test_browser_image.json").to_vec();
    let payload: serde_json::Value = serde_json::from_slice(&image).unwrap();
    manifest.builder_adapter = "conduit-host-browser/bind-prebuilt@1".into();
    manifest.files.push(HostReleaseFile {
        path: "conduit-browser-image.json".into(),
        bytes: image.len() as u64,
        sha256: digest(&image),
        media_type: "application/vnd.conduit.browser-image+json".into(),
    });
    let distribution = manifest.bundle_sha256.clone();
    let mut aggregate = Sha256::new();
    aggregate.update(b"conduit.release/host-bundle-content@1\0");
    for file in &manifest.files {
        aggregate.update(file.path.as_bytes());
        aggregate.update(b"\0");
        aggregate.update(file.sha256.as_bytes());
        aggregate.update(b"\n");
    }
    manifest.bundle_sha256 = format!("sha256:{:x}", aggregate.finalize());
    manifest.target_metadata.clear();
    for (key, value) in [
        ("browser_image_id", payload["image_id"].clone()),
        ("browser_build_id", payload["build_id"].clone()),
        ("browser_profile_id", payload["profile_id"].clone()),
        (
            "browser_configuration_id",
            payload["source_configuration_id"].clone(),
        ),
        (
            "distribution_id",
            "conduit.browser/reviewed-distribution@1".into(),
        ),
        ("distribution_sha256", distribution.into()),
    ] {
        manifest.target_metadata.insert(key.into(), value);
    }
    (serde_json::to_vec(&manifest).unwrap(), image)
}
