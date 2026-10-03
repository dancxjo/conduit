use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use clap::ValueEnum;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[path = "host_release_browser.rs"]
mod browser;

const RELEASE_SCHEMA: &str = "conduit.release/host-bundle@1";
// Browser consumers retain their 32 MiB file limit. Installed native Hosts
// accept 64 MiB release files, so sealing must admit the same finite range.
const MAXIMUM_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAXIMUM_NATIVE_FILE_BYTES: u64 = 64 * 1024 * 1024;

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
    make_package_id: &'a str,
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
        Command::new("cargo")
            .args([
                "build",
                "--locked",
                "--release",
                "-p",
                "conduit-browser-runtime",
                "--no-default-features",
                "--features",
                "plot-runner,creche-surface",
                "--target",
                "wasm32-unknown-unknown",
            ])
            // Keep the retained browser product compact across its dependency graph.
            .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "z")
            .env("CARGO_PROFILE_RELEASE_LTO", "thin"),
        "compile browser Host release",
    )?;
    fs::create_dir_all(output)?;
    copy(
        cargo_release_artifact(
            Some("wasm32-unknown-unknown"),
            "conduit_browser_runtime.wasm",
        ),
        &output.join("runtime.wasm"),
    )?;
    let browser_files = [
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
            "targets/browser/host/assets/browser-runtime-bridge.mjs",
            "browser-runtime-bridge.mjs",
            "text/javascript; charset=utf-8",
        ),
        (
            "targets/browser/host/assets/browser-relay-line.mjs",
            "browser-relay-line.mjs",
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
    let mut manifest_files = vec![("runtime.wasm", "application/wasm")];
    manifest_files.extend(browser_files.map(|(_, name, media)| (name, media)));
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
        Command::new("aarch64-linux-gnu-gcc").arg("--version"),
        "Raspberry Pi OS C cross compiler unavailable; run `cargo xtask setup linux-release`",
    )?;
    require_success(
        Command::new("aarch64-linux-gnu-g++").arg("--version"),
        "Raspberry Pi OS C++ cross compiler unavailable; run `cargo xtask setup linux-release`",
    )?;
    require_success(
        Command::new("cmake").arg("--version"),
        "CMake unavailable; run `cargo xtask setup linux-release`",
    )?;
    require_success(
        Command::new("cargo").args(["build", "--locked", "--release", "-p", "conduit"]),
        "compile hosted Linux release",
    )?;
    fs::create_dir_all(output)?;
    copy(
        cargo_release_artifact(None, "conduit"),
        &output.join("conduit-linux-x86_64"),
    )?;
    copy(
        "products/conduit/install/install-linux-x86_64.sh",
        &output.join("install-linux-x86_64.sh"),
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
        cargo_release_artifact(Some("aarch64-unknown-linux-gnu"), "conduit"),
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
        &[
            (
                "conduit-linux-x86_64",
                "application/vnd.conduit.host+executable",
            ),
            ("install-linux-x86_64.sh", "application/x-sh"),
        ],
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
        cargo_release_artifact(None, "conduit.exe"),
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
        cargo_release_artifact(None, "conduit"),
        &output.join("conduit-macos-aarch64"),
    )?;
    copy(
        "products/conduit/install/install-macos-aarch64.sh",
        &output.join("install-macos-aarch64.sh"),
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
        &[
            (
                "conduit-macos-aarch64",
                "application/vnd.conduit.host+executable",
            ),
            ("install-macos-aarch64.sh", "application/x-sh"),
        ],
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
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAXIMUM_NATIVE_FILE_BYTES
        {
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
        make_package_id: package_id,
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

fn cargo_release_artifact(target: Option<&str>, binary: &str) -> PathBuf {
    // The Cargo child inherits this variable and the same working directory.
    let mut artifact = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"));
    if let Some(target) = target {
        artifact.push(target);
    }
    artifact.join("release").join(binary)
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!("sha256:{:x}", Sha256::digest(fs::read(path)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_release_seal_accepts_installer_sized_product_and_refuses_larger_file() {
        let root = std::env::temp_dir().join(format!(
            "conduit-native-release-bound-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let product = root.join("conduit-linux-x86_64");
        let file = fs::File::create(&product).unwrap();
        file.set_len(MAXIMUM_FILE_BYTES + 1).unwrap();
        drop(file);
        let seal_product = |manifest_name| {
            seal(
                &root,
                manifest_name,
                "std/x86_64/computer",
                "hosted-native@1",
                "native-bundle",
                "test/build",
                "test/launch",
                "test-source",
                &[(
                    "conduit-linux-x86_64",
                    "application/vnd.conduit.host+executable",
                )],
            )
        };
        seal_product("release.json").unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("release.json")).unwrap()).unwrap();
        assert_eq!(manifest["files"][0]["bytes"], MAXIMUM_FILE_BYTES + 1);

        fs::OpenOptions::new()
            .write(true)
            .open(&product)
            .unwrap()
            .set_len(MAXIMUM_NATIVE_FILE_BYTES + 1)
            .unwrap();
        let refusal = seal_product("oversized.json").unwrap_err();
        assert!(refusal.to_string().contains("finite byte bound"));
        assert!(!root.join("oversized.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
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
