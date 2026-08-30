use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    avr_toolchain::{avr_gcc_bin, config_path, provision, verify_cores},
    metric, require_success,
};

pub(super) const RUST_TOOLCHAIN: &str = "nightly-2026-05-25";
pub(super) const AVR_HAL_REVISION: &str = "e0b0105b11a7c4209fb1704276a7921c3139d5cb";
pub(super) const FIRMWARE: &str = "targets/avr/firmware/promicro-host";
const ELF_NAME: &str = "conduit-avr-promicro-host.elf";
const HEX_NAME: &str = "conduit-avr-promicro-host.hex";

pub(super) struct RustFirmwareArtifact {
    pub(super) hex: PathBuf,
    pub(super) flash_bytes: u64,
    pub(super) sram_bytes: u64,
}

pub(super) fn provision_rust() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("rustup")
        .args([
            "toolchain",
            "install",
            RUST_TOOLCHAIN,
            "--profile",
            "minimal",
            "--component",
            "rust-src",
        ])
        .output()?;
    require_success(&output, "pinned Rust AVR toolchain install")
}

pub(super) fn build(root: &Path) -> Result<RustFirmwareArtifact, Box<dyn std::error::Error>> {
    provision_rust()?;
    let cli = provision(root)?;
    verify_cores(&cli, root)?;
    let gcc_bin = avr_gcc_bin(root);
    let path = env::join_paths(
        std::iter::once(gcc_bin.clone())
            .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
    )?;
    let manifest = root.join(FIRMWARE).join("Cargo.toml");
    let output = Command::new("rustup")
        .args([
            "run",
            RUST_TOOLCHAIN,
            "cargo",
            "build",
            "--release",
            "--locked",
        ])
        .arg("--manifest-path")
        .arg(&manifest)
        .current_dir(root.join(FIRMWARE))
        .env("PATH", path)
        .output()?;
    require_success(&output, "Rust AVR firmware build")?;

    let firmware_target = root.join(FIRMWARE).join("target/avr-none/release");
    let elf = firmware_target.join(ELF_NAME);
    if !elf.is_file() {
        return Err(format!("Rust AVR build omitted {}", elf.display()).into());
    }
    let output_dir = root.join("target/avr-promicro/build");
    fs::create_dir_all(&output_dir)?;
    let hex = output_dir.join(HEX_NAME);
    let objcopy = gcc_bin.join("avr-objcopy");
    let output = Command::new(objcopy)
        .args(["-O", "ihex", "-R", ".eeprom"])
        .arg(&elf)
        .arg(&hex)
        .output()?;
    require_success(&output, "Rust AVR HEX conversion")?;

    let size = Command::new(gcc_bin.join("avr-size"))
        .args(["-C", "--mcu=atmega32u4"])
        .arg(&elf)
        .output()?;
    require_success(&size, "Rust AVR size accounting")?;
    let report = String::from_utf8(size.stdout)?;
    Ok(RustFirmwareArtifact {
        hex,
        flash_bytes: metric(&report, "Program:", "bytes")?,
        sram_bytes: metric(&report, "Data:", "bytes")?,
    })
}

pub(super) fn check(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    provision_rust()?;
    let cli = provision(root)?;
    verify_cores(&cli, root)?;
    let gcc = avr_gcc_bin(root).join("avr-gcc");
    if !gcc.is_file() {
        return Err(format!("pinned AVR GCC is absent at {}", gcc.display()).into());
    }
    if !root.join(FIRMWARE).join("Cargo.lock").is_file() {
        return Err("Rust AVR firmware lockfile is absent".into());
    }
    let _ = config_path(root);
    Ok(())
}
