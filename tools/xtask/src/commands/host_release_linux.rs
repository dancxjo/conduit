//! Existing x86_64 and AArch64 Linux release set.
use super::*;
use std::io::Read;
#[path = "host_release_linux_native.rs"]
mod native;

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
    compile_x86_64(output, false, false)?;
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
    seal_x86_64(output, source_identity)?;
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

/// Exact 64-bit Linux Pi deliveries, built off-device without an x86 build.
pub(super) fn build_aarch64(
    output: &Path,
    source_identity: &str,
    native_display: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    require_success(
        Command::new("aarch64-linux-gnu-gcc").arg("--version"),
        "AArch64 C cross compiler",
    )?;
    require_success(
        &mut release_command(Some("aarch64-unknown-linux-gnu"), native_display)?,
        "compile AArch64 Linux Host",
    )?;
    fs::create_dir_all(output)?;
    copy(
        cargo_release_artifact(Some("aarch64-unknown-linux-gnu"), "conduit"),
        &output.join("conduit-linux-aarch64"),
    )?;
    for machine in [
        "raspberry-pi-3-model-b-plus-rev-1.3",
        "raspberry-pi-5-model-b-rev-1.1-8gb",
    ] {
        seal(
            output,
            &format!("linux-aarch64-{machine}.json"),
            &format!("std/aarch64/{machine}"),
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

pub(super) fn build_x86_64(
    output: &Path,
    source_identity: &str,
    native_display: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    compile_x86_64(output, native_display, true)?;
    seal_x86_64(output, source_identity)
}
pub(in crate::commands) fn seal_linux_installer(
    executable: &Path,
    output: &Path,
    source_identity: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = fs::metadata(executable)?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_NATIVE_FILE_BYTES {
        return Err("installer executable violates its finite file bound".into());
    }
    let mut header = [0; 20];
    fs::File::open(executable)?.read_exact(&mut header)?;
    // Current Linux executables are ELF64 little endian, including PIE.
    if header[..6] != *b"\x7fELF\x02\x01"
        || !matches!(u16::from_le_bytes([header[16], header[17]]), 2 | 3)
        || u16::from_le_bytes([header[18], header[19]]) != 62
    {
        return Err("installer requires an ELF Linux x86-64 executable".into());
    }
    fs::create_dir(output)?;
    stage_x86_64(executable, output)?;
    seal_x86_64(output, source_identity)
}

fn compile_x86_64(
    output: &Path,
    native_display: bool,
    explicit_target: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = explicit_target.then_some("x86_64-unknown-linux-gnu");
    require_success(
        &mut release_command(target, native_display)?,
        "compile hosted Linux release",
    )?;
    fs::create_dir_all(output)?;
    stage_x86_64(&cargo_release_artifact(target, "conduit"), output)
}

fn stage_x86_64(executable: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    copy(executable, &output.join("conduit-linux-x86_64"))?;
    copy(
        "products/conduit/install/install-linux-x86_64.sh",
        &output.join("install-linux-x86_64.sh"),
    )
}
fn seal_x86_64(output: &Path, source_identity: &str) -> Result<(), Box<dyn std::error::Error>> {
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
fn release_command(
    target: Option<&str>,
    native_display: bool,
) -> Result<Command, Box<dyn std::error::Error>> {
    let search = if native_display {
        Some(native::linker_search(target)?)
    } else {
        None
    };
    Ok(command_for(target, native_display, search.as_deref()))
}
fn command_for(
    target: Option<&str>,
    native_display: bool,
    search: Option<&std::ffi::OsStr>,
) -> Command {
    let mut command = Command::new("cargo");
    if target == Some("aarch64-unknown-linux-gnu") {
        command.arg("+1.98.1");
    }
    command.args(["build", "--locked", "--release", "-p", "conduit"]);
    if let Some(target) = target {
        command.args(["--target", target]);
        if target == "aarch64-unknown-linux-gnu" {
            command.env(
                "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER",
                "aarch64-linux-gnu-gcc",
            );
        }
    }
    if native_display {
        command.args(["--features", "native-display"]);
    }
    if let Some(search) = search {
        command.env("LIBRARY_PATH", search);
    }
    command
}
#[cfg(test)]
#[path = "host_release_linux/tests.rs"]
mod tests;
