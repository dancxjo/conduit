//! Hosted Linux and Raspberry Pi OS release manufacture.
use super::{cargo_release_artifact, copy, require_success, seal};
use std::{fs, path::Path, process::Command};

pub(super) fn build_linux_set(
    output: &Path,
    source_identity: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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
    build_linux_x86_64(output, source_identity)?;
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

/// Native x86-64 release only; no ARM cross-SDL capability is implied.
pub(super) fn build_linux_x86_64(
    output: &Path,
    source_identity: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err(
            "Linux x86-64 release manufacture requires a Linux x86-64 development Host".into(),
        );
    }
    require_success(
        Command::new("pkg-config").args(["--atleast-version=2.0.0", "sdl2"]),
        "native SDL2 development files unavailable; run `cargo xtask setup linux-release`",
    )?;
    require_success(
        Command::new("cargo").args([
            "build",
            "--locked",
            "--release",
            "-p",
            "conduit",
            "--bin",
            "conduit",
            "--features",
            "native-creche-sdl",
            // Optimize this Linux CLI for the existing installed-file budget.
            // Keep dependency optimization and panic/unwind recovery unchanged.
            "--config",
            "profile.release.package.conduit.opt-level=\"s\"",
            "--target",
            "x86_64-unknown-linux-gnu",
        ]),
        "compile hosted Linux release",
    )?;
    fs::create_dir_all(output)?;
    copy(
        cargo_release_artifact(Some("x86_64-unknown-linux-gnu"), "conduit"),
        &output.join("conduit-linux-x86_64"),
    )?;
    copy(
        "products/conduit/install/install-linux-x86_64.sh",
        &output.join("install-linux-x86_64.sh"),
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
    Ok(())
}
