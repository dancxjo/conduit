use std::{fs, path::Path, process::Command};

use clap::ValueEnum;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[path = "host_release_browser.rs"]
mod browser;

const RELEASE_SCHEMA: &str = "conduit.release/host-bundle@1";
const MAXIMUM_FILE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum ReleasePlatform {
    Browser,
    Linux,
    Windows,
    Macos,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    reviewed_distribution: Option<browser::ReviewedBrowserDistribution<'a>>,
}

#[derive(Serialize)]
struct ReleaseFile {
    path: String,
    bytes: u64,
    sha256: String,
    media_type: &'static str,
}

pub(crate) struct ReleaseOptions {
    pub(crate) json: bool,
    pub(crate) quiet: bool,
}

pub(crate) fn run(
    output: &Path,
    platform: ReleasePlatform,
    source_identity: &str,
    opts: &ReleaseOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    match platform {
        ReleasePlatform::Browser => build_browser(output, source_identity)?,
        ReleasePlatform::Linux => build_linux_set(output, source_identity)?,
        ReleasePlatform::Windows => build_windows(output, source_identity)?,
        ReleasePlatform::Macos => build_macos(output, source_identity)?,
    }

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

fn build_browser(output: &Path, source_identity: &str) -> Result<(), Box<dyn std::error::Error>> {
    require_success(
        Command::new("cargo").args([
            "build",
            "--locked",
            "--release",
            "-p",
            "conduit-browser-runtime",
            "--no-default-features",
            "--features",
            "form-runner",
            "--target",
            "wasm32-unknown-unknown",
        ]),
        "compile browser Host release",
    )?;
    fs::create_dir_all(output)?;
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
            "targets/browser/host/assets/browser-host-membership.mjs",
            "browser-host-membership.mjs",
            "text/javascript; charset=utf-8",
        ),
        (
            "targets/browser/host/assets/browser-host-identity.mjs",
            "browser-host-identity.mjs",
            "text/javascript; charset=utf-8",
        ),
        (
            "targets/browser/host/assets/browser-boot-profile.mjs",
            "browser-boot-profile.mjs",
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
    let manifest_files = browser_files.map(|(_, name, media)| (name, media));
    browser::seal(
        output,
        "browser-page.json",
        "browser/wasm32/page",
        "browser-wasm@1",
        "browser-bundle",
        "conduit-host-browser/build-wasm@1",
        "conduit-host-browser/load@1",
        source_identity,
        &manifest_files,
    )
}

fn build_linux_set(output: &Path, source_identity: &str) -> Result<(), Box<dyn std::error::Error>> {
    require_success(
        Command::new("cargo").args(["build", "--locked", "--release", "-p", "conduit"]),
        "compile hosted Linux release",
    )?;
    fs::create_dir_all(output)?;
    copy(
        "target/release/conduit",
        &output.join("conduit-linux-x86_64"),
    )?;
    require_success(
        Command::new("cargo")
            .args([
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
    for (manifest, target_id) in [
        (
            "raspios-bookworm-zero-2-w-rev-1.0.json",
            "std/aarch64/raspberry-pi-zero-2-w-rev-1.0",
        ),
        (
            "raspios-bookworm-zero-2-wh-rev-1.0.json",
            "std/aarch64/raspberry-pi-zero-2-wh-rev-1.0",
        ),
    ] {
        seal(
            output,
            manifest,
            target_id,
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
    }

    Ok(())
}

fn build_windows(output: &Path, source_identity: &str) -> Result<(), Box<dyn std::error::Error>> {
    require_success(
        Command::new("cargo").args(["build", "--locked", "--release", "-p", "conduit"]),
        "compile hosted Windows x86_64 release",
    )?;
    fs::create_dir_all(output)?;
    copy(
        "target/release/conduit.exe",
        &output.join("conduit-windows-x86_64.exe"),
    )?;
    seal(
        output,
        "hosted-windows-x86_64.json",
        "std/x86_64/windows-computer",
        "hosted-native@1",
        "native-bundle",
        "conduit-host-hosted/build-native@1",
        "conduit-host-hosted/launch@1",
        source_identity,
        &[(
            "conduit-windows-x86_64.exe",
            "application/vnd.microsoft.portable-executable",
        )],
    )
}

fn build_macos(output: &Path, source_identity: &str) -> Result<(), Box<dyn std::error::Error>> {
    require_success(
        Command::new("cargo").args(["build", "--locked", "--release", "-p", "conduit"]),
        "compile hosted macOS aarch64 release",
    )?;
    fs::create_dir_all(output)?;
    copy(
        "target/release/conduit",
        &output.join("conduit-macos-aarch64"),
    )?;
    seal(
        output,
        "hosted-macos-aarch64.json",
        "std/aarch64/macos-computer",
        "hosted-native@1",
        "native-bundle",
        "conduit-host-hosted/build-native@1",
        "conduit-host-hosted/launch@1",
        source_identity,
        &[(
            "conduit-macos-aarch64",
            "application/vnd.conduit.host+executable",
        )],
    )
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
        reviewed_distribution: None,
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
