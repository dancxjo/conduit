use std::{fs, path::Path};

use serde::Serialize;

use super::{
    bundle_digest, sha256_file, ReleaseFile, ReleaseManifest, MAXIMUM_FILE_BYTES, RELEASE_SCHEMA,
};

#[derive(Serialize)]
pub(super) struct ReviewedBrowserDistribution<'a> {
    schema: &'static str,
    distribution_id: &'static str,
    runtime_abi: &'static str,
    targets: [&'static str; 1],
    toolchain_identity: &'static str,
    source_commit: &'a str,
    maximum_bundle_bytes: u64,
    implementations: Vec<ReviewedBrowserImplementation<'a>>,
    modules: [ReviewedBrowserModule<'static>; 8],
}

#[derive(Serialize)]
struct ReviewedBrowserImplementation<'a> {
    id: &'a str,
    revision: u32,
    artifact: &'a str,
}

#[derive(Serialize)]
struct ReviewedBrowserModule<'a> {
    path: &'a str,
    dependencies: &'a [&'a str],
}

#[allow(clippy::too_many_arguments)]
pub(super) fn seal(
    root: &Path,
    manifest_name: &str,
    target_id: &str,
    package_id: &str,
    output: &str,
    builder: &str,
    deployment: &str,
    source_identity: &str,
    files: &[(&str, &'static str)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut entries = Vec::with_capacity(files.len());
    for (name, media_type) in files {
        let path = root.join(name);
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAXIMUM_FILE_BYTES {
            return Err(format!(
                "release file {} violates its finite byte bound",
                path.display()
            )
            .into());
        }
        entries.push(ReleaseFile {
            path: (*name).into(),
            bytes: metadata.len(),
            sha256: sha256_file(&path)?,
            media_type,
        });
    }
    let bundle_sha256 = bundle_digest(&entries);
    let implementations = conduit_host_browser_fabrication::BROWSER_IMPLEMENTATIONS
        .iter()
        .map(|item| ReviewedBrowserImplementation {
            id: item.implementation_id,
            revision: item.implementation_revision,
            artifact: item.artifact,
        })
        .collect();
    let manifest = ReleaseManifest {
        schema: RELEASE_SCHEMA,
        target_id,
        fabrication_package_id: package_id,
        output,
        builder_adapter: builder,
        deployment_adapter: deployment,
        source_identity,
        bundle_sha256: format!("sha256:{bundle_sha256}"),
        files: entries,
        reviewed_distribution: Some(ReviewedBrowserDistribution {
            schema: "conduit.browser/reviewed-distribution@1",
            distribution_id: conduit_host_browser_fabrication::REVIEWED_DISTRIBUTION_ID,
            runtime_abi: "conduit.browser/runtime-abi@1",
            targets: ["browser/wasm32/page"],
            toolchain_identity: "rustc:stable+wasm32-unknown-unknown",
            source_commit: source_identity,
            maximum_bundle_bytes: 48 * 1024 * 1024,
            implementations,
            modules: [
                module("host.mjs", &["browser-host-bootstrap.mjs"]),
                module(
                    "browser-host-bootstrap.mjs",
                    &["browser-host-membership.mjs"],
                ),
                module(
                    "browser-host-membership.mjs",
                    &["browser-host-identity.mjs"],
                ),
                module("browser-host-identity.mjs", &[]),
                module("browser-boot-profile.mjs", &[]),
                module("media-host.mjs", &[]),
                module("device-base.mjs", &[]),
                module("usb-device-base.mjs", &[]),
            ],
        }),
    };
    fs::write(
        root.join(manifest_name),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

const fn module(
    path: &'static str,
    dependencies: &'static [&'static str],
) -> ReviewedBrowserModule<'static> {
    ReviewedBrowserModule { path, dependencies }
}
