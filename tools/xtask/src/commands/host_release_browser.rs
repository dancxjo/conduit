use std::{fs, path::Path, process::Command};

use serde::Serialize;

use super::{
    bundle_digest, sha256_file, ReleaseFile, ReleaseManifest, MAXIMUM_FILE_BYTES, RELEASE_SCHEMA,
};

/// Optimize the final linked browser module before any digest or package is sealed.
/// Keep the Cargo artifact intact for diagnostics; failure never falls back to it.
pub(super) fn optimize_runtime(
    source: &Path,
    destination: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let linked_bytes = fs::metadata(source)?.len();
    super::require_success(
        Command::new("wasm-opt").arg("--version"),
        "identify Binaryen browser optimizer (install wasm-opt if missing)",
    )?;
    // Stage beside the destination so validated publication is one rename.
    // A failed optimizer or validation leaves the previous package untouched.
    let staged = destination.with_extension(format!("{}.optimized.wasm", std::process::id()));
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)?;
    let result = (|| {
        super::require_success(
            Command::new("wasm-opt")
                .args(["-Oz", "--enable-bulk-memory", "--enable-sign-ext"])
                .arg(source)
                .arg("-o")
                .arg(&staged)
                .env("BINARYEN_CORES", "2"),
            "optimize browser Host release",
        )?;
        let bytes = publish_runtime(&staged, destination)?;
        println!("Browser runtime: {linked_bytes} linked bytes -> {bytes} optimized bytes");
        Ok(())
    })();
    if staged.exists() {
        fs::remove_file(&staged)?;
    }
    result
}

fn publish_runtime(staged: &Path, destination: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    use std::io::Read;
    // Match the browser SDK, bootstrap and installed Host admission ceiling.
    const MAXIMUM_RUNTIME_BYTES: u64 = 20 * 1024 * 1024;
    let bytes = fs::metadata(staged)?.len();
    if !(8..=MAXIMUM_RUNTIME_BYTES).contains(&bytes) {
        return Err(format!("optimized browser runtime has {bytes} bytes; its finite file bound is 8..={MAXIMUM_RUNTIME_BYTES}").into());
    }
    let mut header = [0; 8];
    fs::File::open(staged)?.read_exact(&mut header)?;
    if header != *b"\0asm\x01\0\0\0" {
        return Err("optimized browser runtime is not a WebAssembly module".into());
    }
    fs::rename(staged, destination)?;
    Ok(bytes)
}

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
    modules: [ReviewedBrowserModule<'static>; 10],
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
    let implementations = conduit_host_browser_make::BROWSER_IMPLEMENTATIONS
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
        make_package_id: package_id,
        output,
        builder_adapter: builder,
        deployment_adapter: deployment,
        source_identity,
        bundle_sha256: format!("sha256:{bundle_sha256}"),
        files: entries,
        reviewed_distribution: Some(ReviewedBrowserDistribution {
            schema: "conduit.browser/reviewed-distribution@1",
            distribution_id: conduit_host_browser_make::REVIEWED_DISTRIBUTION_ID,
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
                module("browser-runtime-bridge.mjs", &[]),
                module("browser-relay-line.mjs", &[]),
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

#[cfg(test)]
mod runtime_publication_tests {
    use super::*;

    #[test]
    fn publication_matches_browser_bound_and_preserves_previous_bytes_on_refusal() {
        let root = std::env::temp_dir().join(format!(
            "conduit-runtime-publication-{}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let staged = root.join("optimized.wasm");
        let destination = root.join("runtime.wasm");
        let original = b"previous packaged runtime";
        fs::write(&destination, original).unwrap();
        for bytes in [
            b"".as_slice(),
            b"\0asm".as_slice(),
            b"not wasm".as_slice(),
            b"\0asm\x02\0\0\0".as_slice(),
        ] {
            fs::write(&staged, bytes).unwrap();
            assert!(publish_runtime(&staged, &destination).is_err());
            assert_eq!(fs::read(&destination).unwrap(), original);
        }
        fs::write(&staged, b"\0asm\x01\0\0\0").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&staged)
            .unwrap()
            .set_len(20 * 1024 * 1024 + 1)
            .unwrap();
        assert!(publish_runtime(&staged, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), original);
        // Exact consumer ceiling is admitted, then a second publication replaces it.
        fs::OpenOptions::new()
            .write(true)
            .open(&staged)
            .unwrap()
            .set_len(20 * 1024 * 1024)
            .unwrap();
        assert_eq!(
            publish_runtime(&staged, &destination).unwrap(),
            20 * 1024 * 1024
        );
        assert!(!staged.exists());
        fs::write(&staged, b"\0asm\x01\0\0\0").unwrap();
        assert_eq!(publish_runtime(&staged, &destination).unwrap(), 8);
        assert_eq!(fs::read(&destination).unwrap(), b"\0asm\x01\0\0\0");
        fs::remove_dir_all(root).unwrap();
    }
}
