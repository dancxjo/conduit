use std::{fs, path::Path, process::Command};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli::GlobalOpts;

const RELEASE_SCHEMA: &str = "conduit.release/host-bundle@1";
const MAXIMUM_FILE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Serialize)]
struct ReleaseManifest<'a> {
    schema: &'static str,
    target_id: &'a str,
    fabrication_package_id: &'a str,
    output: &'a str,
    builder_adapter: &'a str,
    deployment_adapter: &'a str,
    source_identity: &'a str,
    bundle_sha256: String,
    files: Vec<ReleaseFile>,
}

#[derive(Serialize)]
struct ReleaseFile {
    path: String,
    bytes: u64,
    sha256: String,
    media_type: &'static str,
}

pub(super) fn run(
    output: &Path,
    source_identity: &str,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    require_success(
        Command::new("cargo").args(["+stable", "build", "--locked", "--release", "-p", "conduit"]),
        "compile hosted Linux release",
    )?;
    require_success(
        Command::new("cargo").args([
            "+stable",
            "build",
            "--locked",
            "--release",
            "-p",
            "conduit-browser-runtime",
            "--target",
            "wasm32-unknown-unknown",
        ]),
        "compile browser Host release",
    )?;

    fs::create_dir_all(output)?;
    copy(
        "target/release/conduit",
        &output.join("conduit-linux-x86_64"),
    )?;
    require_success(
        Command::new("cargo")
            .args([
                "+stable",
                "build",
                "--locked",
                "--release",
                "-p",
                "conduit",
                "--target",
                "aarch64-unknown-linux-gnu",
            ])
            .env(
                "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER",
                "aarch64-linux-gnu-gcc",
            ),
        "compile Raspberry Pi OS aarch64 release",
    )?;
    copy(
        "target/aarch64-unknown-linux-gnu/release/conduit",
        &output.join("conduit-linux-aarch64"),
    )?;
    seal(
        output,
        "hosted-linux-x86_64.json",
        "std/x86_64/computer",
        "hosted-native@1",
        "native-bundle",
        "conduit-host-hosted/build-native@1",
        "conduit-host-hosted/launch@1",
        source_identity,
        &[(
            "conduit-linux-x86_64",
            "application/vnd.conduit.host+executable",
        )],
    )?;
    seal(
        output,
        "raspios-bookworm-pi4-model-b-rev-1.5-4gb.json",
        "std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb",
        "conduit-host-raspberry-pi@1",
        "native-bundle",
        "conduit-host-raspberry-pi/build-raspios-native@1",
        "conduit-host-raspberry-pi/install-raspios-package@1",
        source_identity,
        &[(
            "conduit-linux-aarch64",
            "application/vnd.conduit.host+executable",
        )],
    )?;

    let browser_files = [
        (
            "target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
            "runtime.wasm",
            "application/wasm",
        ),
        (
            "targets/browser/host/assets/index.html",
            "index.html",
            "text/html; charset=utf-8",
        ),
        (
            "targets/browser/host/assets/host.mjs",
            "host.mjs",
            "text/javascript; charset=utf-8",
        ),
        (
            "targets/browser/host/assets/browser-host-bootstrap.mjs",
            "browser-host-bootstrap.mjs",
            "text/javascript; charset=utf-8",
        ),
        (
            "targets/browser/host/assets/media-host.mjs",
            "media-host.mjs",
            "text/javascript; charset=utf-8",
        ),
        (
            "targets/browser/host/assets/device-base.mjs",
            "device-base.mjs",
            "text/javascript; charset=utf-8",
        ),
        (
            "targets/browser/host/assets/usb-device-base.mjs",
            "usb-device-base.mjs",
            "text/javascript; charset=utf-8",
        ),
    ];
    for (source, name, _) in browser_files {
        copy(source, &output.join(name))?;
    }
    let browser_manifest_files = browser_files.map(|(_, name, media)| (name, media));
    seal(
        output,
        "browser-page.json",
        "browser/wasm32/page",
        "browser-wasm@1",
        "browser-bundle",
        "conduit-host-browser/build-wasm@1",
        "conduit-host-browser/load@1",
        source_identity,
        &browser_manifest_files,
    )?;

    if opts.json {
        println!("{{\"schema\":\"conduit.release/host-bundle-set@1\",\"output\":{:?},\"source_identity\":{:?}}}", output.display().to_string(), source_identity);
    } else if !opts.quiet {
        println!(
            "SEALED existing-computer Host releases in {}",
            output.display()
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn seal(
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
    };
    fs::write(
        root.join(manifest_name),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

fn copy(source: impl AsRef<Path>, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::copy(source, destination)?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!("sha256:{:x}", Sha256::digest(fs::read(path)?)))
}

fn bundle_digest(files: &[ReleaseFile]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"conduit.release/host-bundle-content@1\0");
    for file in files {
        digest.update(file.path.as_bytes());
        digest.update(b"\0");
        digest.update(file.sha256.as_bytes());
        digest.update(b"\n");
    }
    format!("{:x}", digest.finalize())
}

fn require_success(command: &mut Command, label: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} failed with {status}").into())
    }
}
