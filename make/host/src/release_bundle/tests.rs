use super::*;
use std::{cell::Cell, collections::BTreeMap, io::Cursor};

fn payloads() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([
        ("runtime.wasm".into(), b"\0asm\x01\0\0\0".to_vec()),
        (
            "browser-boot-profile.mjs".into(),
            b"export const fixture = true;".to_vec(),
        ),
    ])
}
fn manifest() -> serde_json::Value {
    let files=payloads().into_iter().map(|(path,bytes)|serde_json::json!({"path":path,"bytes":bytes.len(),"sha256":format!("sha256:{:x}",Sha256::digest(bytes)),"media_type":"application/octet-stream"})).collect::<Vec<_>>();
    let mut result = serde_json::json!({
        "schema":"conduit.release/host-bundle@1","target_id":"browser/wasm32/page","make_package_id":"browser-wasm@1","output":"browser-bundle",
        "builder_adapter":"conduit-host-browser/bind-prebuilt@1","deployment_adapter":"conduit-host-browser/load@1","source_identity":"source/prior-make",
        "bundle_sha256":"","files":files,
        "browser_profile_id":format!("sha256:{}","1".repeat(64)),"browser_configuration_id":format!("sha256:{}","2".repeat(64)),
        "reviewed_distribution":{"schema":"conduit.browser/reviewed-distribution@1","runtime_abi":"conduit.browser/runtime-abi@1"}
    });
    aggregate(&mut result);
    result
}
fn aggregate(value: &mut serde_json::Value) {
    // This is the existing JS/xtask identity encoding, not raw payload bytes.
    let mut source = "conduit.release/host-bundle-content@1\0".to_string();
    for file in value["files"].as_array().unwrap() {
        source.push_str(file["path"].as_str().unwrap());
        source.push('\0');
        source.push_str(file["sha256"].as_str().unwrap());
        source.push('\n');
    }
    value["bundle_sha256"] = format!("sha256:{:x}", Sha256::digest(source)).into();
}
fn expected() -> ReleaseTargetProfile<'static> {
    ReleaseTargetProfile {
        target_id: "browser/wasm32/page",
        package_id: "browser-wasm@1",
        output: "browser-bundle",
        builder_adapter: "conduit-host-browser/bind-prebuilt@1",
        deployment_adapter: "conduit-host-browser/load@1",
    }
}
fn bounds() -> HostReleaseBounds {
    HostReleaseBounds {
        maximum_manifest_bytes: 65536,
        maximum_files: 16,
        maximum_file_bytes: 1024,
        maximum_bundle_bytes: 2048,
    }
}
fn verify(
    value: &serde_json::Value,
    files: BTreeMap<String, Vec<u8>>,
) -> Result<VerifiedHostReleaseBytes, String> {
    verify_host_release_bytes(
        &serde_json::to_vec(value).unwrap(),
        &expected(),
        "source/prior-make",
        bounds(),
        |path| {
            files
                .get(path)
                .cloned()
                .map(Cursor::new)
                .ok_or("file unavailable".into())
        },
        || false,
    )
}

#[test]
fn byteproof_preserves_actual_release_shape_and_distinguishes_aggregate_from_raw_file() {
    let manifest = manifest();
    let verified = verify(&manifest, payloads()).unwrap();
    assert_eq!(
        verified.image_content_digest(),
        manifest["bundle_sha256"].as_str().unwrap()
    );
    assert_eq!(verified.payload_bytes(), 36);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(verified.manifest_bytes()).unwrap(),
        manifest
    );
    assert!(verified
        .manifest()
        .target_metadata
        .contains_key("browser_configuration_id"));
    assert_ne!(
        verified.image_content_digest(),
        format!("sha256:{:x}", Sha256::digest(&payloads()["runtime.wasm"]))
    );
}
#[test]
fn changed_missing_truncated_and_extra_payload_bytes_refuse() {
    for variant in 0..4 {
        let mut files = payloads();
        match variant {
            0 => {
                files.get_mut("runtime.wasm").unwrap()[0] = 9;
            }
            1 => {
                files.remove("runtime.wasm");
            }
            2 => {
                files.get_mut("runtime.wasm").unwrap().pop();
            }
            _ => {
                files.get_mut("runtime.wasm").unwrap().push(0);
            }
        }
        assert!(verify(&manifest(), files).is_err());
    }
}
#[test]
fn ambiguous_unsafe_reordered_and_wrong_source_inventory_refuse() {
    for variant in 0..6 {
        let mut value = manifest();
        match variant {
            0 => value["files"][0]["path"] = "../outside".into(),
            1 => value["files"][0]["path"] = "https://elsewhere".into(),
            2 => {
                let duplicate = value["files"][0].clone();
                value["files"].as_array_mut().unwrap().push(duplicate);
            }
            3 => value["files"].as_array_mut().unwrap().reverse(),
            4 => value["source_identity"] = "source/other".into(),
            _ => value["builder_adapter"] = "unreviewed/command".into(),
        }
        if variant < 3 {
            aggregate(&mut value);
        }
        assert!(verify(&value, payloads()).is_err());
    }
}
#[test]
fn admission_bounds_and_cancellation_prevent_unbounded_effects() {
    let bytes = serde_json::to_vec(&manifest()).unwrap();
    let opens = Cell::new(0);
    for variant in 0..4 {
        let mut b = bounds();
        match variant {
            0 => b.maximum_manifest_bytes = 1,
            1 => b.maximum_files = 1,
            2 => b.maximum_bundle_bytes = 1,
            _ => b.maximum_file_bytes = 1,
        };
        assert!(verify_host_release_bytes(
            &bytes,
            &expected(),
            "source/prior-make",
            b,
            |_| {
                opens.set(opens.get() + 1);
                Ok(Cursor::new(vec![]))
            },
            || false
        )
        .is_err());
    }
    assert_eq!(opens.get(), 0);
    assert!(verify_host_release_bytes(
        &bytes,
        &expected(),
        "source/prior-make",
        bounds(),
        |_| {
            opens.set(opens.get() + 1);
            Ok(Cursor::new(vec![]))
        },
        || true
    )
    .err()
    .unwrap()
    .contains("cancelled"));
    assert_eq!(opens.get(), 0);
}

#[test]
fn rich_browser_metadata_is_retained_without_claiming_target_verification() {
    let mut value = manifest();
    value["browser_image_id"] = "unverified-target-field".into();
    let result = verify(&value, payloads()).unwrap();
    assert_eq!(
        result.manifest().target_metadata["browser_image_id"],
        "unverified-target-field"
    );
    // This proof has no target-ready, profile, Boot, membership or deployment
    // accessor. A separate target-owned verifier is required before Seed use.
}
