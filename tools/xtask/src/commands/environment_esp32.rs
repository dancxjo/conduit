//! Pinned ESP32 firmware construction; no flash or installed Body claim.
#[path = "environment_packaging.rs"]
mod packaging;

use std::{
    error::Error,
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn build(root: &Path, native: bool, body: bool) -> Result<(), Box<dyn Error>> {
    build_with_owner(root, native, body, None, None)
}
pub(super) fn build_member(root: &Path, key: &Path) -> Result<(), Box<dyn Error>> {
    build_with_owner(root, true, false, Some(key), None)
}
pub(super) fn build_display_fit(root: &Path, key: &Path) -> Result<(), Box<dyn Error>> {
    build_with_owner(root, true, false, Some(key), Some("display-fit-root"))
}
pub(super) fn build_mask_preparation(root: &Path, key: &Path) -> Result<(), Box<dyn Error>> {
    build_with_owner(root, true, false, Some(key), Some("mask-root"))
}
pub(super) fn build_display_check(root: &Path, key: &Path) -> Result<(), Box<dyn Error>> {
    build_with_owner(root, true, false, Some(key), Some("display-physical-check"))
}
pub(super) fn build_wireless_display(root: &Path, key: &Path) -> Result<(), Box<dyn Error>> {
    build_with_owner(
        root,
        true,
        false,
        Some(key),
        Some("display-physical-check,owner-wifi"),
    )
}
fn build_with_owner(
    root: &Path,
    native: bool,
    body: bool,
    key: Option<&Path>,
    profile: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let package = root.join("targets/esp32/proof/tm1637");
    let toolchain: toml::Value = toml::from_str(&std::fs::read_to_string(
        package.join("rust-toolchain.toml"),
    )?)?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|value| value.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or("ESP32 firmware has no pinned toolchain")?;
    let linker = linker(channel)?;
    let mut paths = vec![linker.parent().ok_or("linker directory")?.to_path_buf()];
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    let selected_profile = profile.unwrap_or(if body {
        "body-image-root"
    } else {
        "native-face-root"
    });
    if native {
        packaging::check_packages(&package, channel, selected_profile)?;
    }
    let mut command = Command::new("cargo");
    command
        .current_dir(&package)
        .arg(format!("+{channel}"))
        .args([
            if native { "rustc" } else { "build" },
            "--locked",
            "--release",
        ])
        .env("PATH", std::env::join_paths(paths)?)
        .env("CARGO_TARGET_XTENSA_ESP32_NONE_ELF_LINKER", &linker)
        .env("CARGO_TARGET_DIR", root.join("target/environment-esp32"));
    if native {
        command.args(["--features", selected_profile]);
    }
    if let Some(key) = key {
        command.env(
            "CONDUIT_ENVIRONMENT_OWNER_PUBLIC_KEY_FILE",
            std::fs::canonicalize(key)?,
        );
    }
    if native {
        // ESP32's linker places compiler-generated switch lookup tables in
        // scarce internal RAM. Keep control flow in flash instead; this does
        // not change the allocator partition or the admitted CPU stack floor.
        command.args(["--", "-C", "llvm-args=-switch-to-lookup=false"]);
    }
    super::execute(&mut command)?;
    if native {
        let symbols = linker.with_file_name(format!(
            "xtensa-esp32-elf-nm{}",
            std::env::consts::EXE_SUFFIX
        ));
        packaging::check_linked(
            &symbols,
            &root.join(
                "target/environment-esp32/xtensa-esp32-none-elf/release/conduit-esp32-tm1637-probe",
            ),
        )?;
    }
    Ok(())
}

fn linker(channel: &str) -> Result<PathBuf, Box<dyn Error>> {
    let output = Command::new("rustup")
        .args(["run", channel, "rustc", "--print", "sysroot"])
        .output()?;
    if !output.status.success() {
        return Err("cannot inspect pinned ESP32 toolchain; run its setup first".into());
    }
    let sysroot = PathBuf::from(String::from_utf8(output.stdout)?.trim());
    let directory = sysroot.join("xtensa-esp-elf");
    let executable = format!("xtensa-esp32-elf-gcc{}", std::env::consts::EXE_SUFFIX);
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(&directory)? {
        let candidate = entry?.path().join("xtensa-esp-elf/bin").join(&executable);
        if candidate.is_file() {
            candidates.push(candidate);
        }
    }
    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err("pinned ESP32 toolchain has no installed Xtensa linker".into()),
        _ => Err("pinned ESP32 toolchain has ambiguous Xtensa linkers".into()),
    }
}
