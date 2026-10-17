use std::{fs, path::Path, process::Command};

use serde::Serialize;

use super::{
    bundle_digest, sha256_file, ReleaseFile, ReleaseManifest, MAXIMUM_FILE_BYTES, RELEASE_SCHEMA,
};

/// Finalize the linked Cargo module before sealing or any browser proof consumes it.
/// Browser fixtures and installed hosts use this same canonical artifact path.
pub(super) fn optimize_runtime(
    source: &Path,
    destination: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    super::require_success(
        Command::new("wasm-opt").arg("--version"),
        "identify Binaryen browser optimizer (install wasm-opt if missing)",
    )?;
    let linked_bytes = fs::metadata(source)?.len();
    // A sibling allows atomic replacement on the same filesystem. Reserve it
    // exclusively so concurrent release processes cannot overwrite each other.
    let temporary = source.with_extension(format!("{}.optimized.wasm", std::process::id()));
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        super::require_success(
            Command::new("wasm-opt")
                .args(["-Oz", "--enable-bulk-memory", "--enable-sign-ext"])
                .arg(source)
                .arg("-o")
                .arg(&temporary)
                .env("BINARYEN_CORES", "2"),
            "optimize browser Host release",
        )?;
        let bytes = publish_runtime(source, &temporary, destination)?;
        println!("Browser runtime: {linked_bytes} linked bytes -> {bytes} optimized bytes");
        Ok(())
    })();
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    result
}

fn publish_runtime(
    source: &Path,
    optimized: &Path,
    destination: &Path,
) -> Result<u64, Box<dyn std::error::Error>> {
    use std::io::Read;
    // Match every existing browser runtime consumer's finite 20 MiB admission.
    let bytes = fs::metadata(optimized)?.len();
    if !(8..=20 * 1024 * 1024).contains(&bytes) {
        return Err("optimized browser runtime violates its finite file bound".into());
    }
    let mut header = [0; 8];
    fs::File::open(optimized)?.read_exact(&mut header)?;
    if header != *b"\0asm\x01\0\0\0" {
        return Err("optimized browser runtime is not a WebAssembly module".into());
    }
    fs::rename(optimized, source)?;
    // Copy only after finalization: defaults, fixtures and sealed receipts must
    // all identify the same bytes, including when Cargo reuses a fresh artifact.
    super::copy(source, destination)?;
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
mod tests {
    use super::*;

    #[test]
    fn repeated_runtime_finalization_keeps_default_and_sealed_bytes_equal() {
        let root =
            std::env::temp_dir().join(format!("conduit-browser-custody-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("conduit_browser_runtime.wasm");
        let optimized = root.join("optimized.wasm");
        let sealed = root.join("runtime.wasm");
        fs::write(&source, b"linked diagnostic input").unwrap();
        for bytes in [
            b"\0asm\x01\0\0\0".as_slice(),
            b"\0asm\x01\0\0\0\0\x01\0".as_slice(),
        ] {
            fs::write(&optimized, bytes).unwrap();
            assert_eq!(
                publish_runtime(&source, &optimized, &sealed).unwrap(),
                bytes.len() as u64
            );
            assert_eq!(fs::read(&source).unwrap(), bytes);
            assert_eq!(fs::read(&sealed).unwrap(), bytes);
            assert!(!optimized.exists());
        }
        let retained = fs::read(&source).unwrap();
        for bytes in [b"".as_slice(), b"not wasm".as_slice()] {
            fs::write(&optimized, bytes).unwrap();
            assert!(publish_runtime(&source, &optimized, &sealed).is_err());
            assert_eq!(fs::read(&source).unwrap(), retained);
            assert_eq!(fs::read(&sealed).unwrap(), retained);
        }
        let file = fs::File::create(&optimized).unwrap();
        file.set_len(20 * 1024 * 1024 + 1).unwrap();
        assert!(publish_runtime(&source, &optimized, &sealed).is_err());
        assert_eq!(fs::read(&source).unwrap(), retained);
        fs::remove_dir_all(root).unwrap();
    }
}
