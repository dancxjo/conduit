//! Existing x86_64 and AArch64 Linux release set.
use super::*;

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
