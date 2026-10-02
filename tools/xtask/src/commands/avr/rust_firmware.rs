use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    avr_toolchain::{avr_gcc_bin, config_path, provision, verify_cores},
    metric, require_success,
};

pub(super) const RUST_TOOLCHAIN: &str = "nightly-2024-07-22";
pub(super) const AVR_HAL_REVISION: &str = "0be252f2a899dbd687a26f8561048ce61854eaae";
pub(super) const FIRMWARE: &str = "targets/avr/firmware/promicro-host";
const ELF_NAME: &str = "conduit-avr-promicro-host.elf";
const HEX_NAME: &str = "conduit-avr-promicro-host.hex";
const RECEIVE_ONLY_BIN: &str = "conduit-avr-receive-only";
const AUDIBLE_PROBE_BIN: &str = "conduit-avr-audible-probe";

pub(super) struct RustFirmwareArtifact {
    pub(super) hex: PathBuf,
    pub(super) flash_bytes: u64,
    pub(super) sram_bytes: u64,
}

pub(super) fn provision_rust() -> Result<(), Box<dyn std::error::Error>> {
    if installed_rust()? {
        return Ok(());
    }
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
    require_success(&output, "pinned Rust AVR toolchain install")?;
    if !installed_rust()? {
        return Err("pinned Rust AVR installation is incomplete".into());
    }
    Ok(())
}

fn installed_rust() -> Result<bool, Box<dyn std::error::Error>> {
    // `which` only inspects installed toolchains; it does not provision one.
    let output = Command::new("rustup")
        .args(["which", "--toolchain", RUST_TOOLCHAIN, "rustc"])
        .output()?;
    if !output.status.success() {
        return Ok(false);
    }
    let rustc = PathBuf::from(String::from_utf8(output.stdout)?.trim());
    let root = rustc
        .parent()
        .and_then(Path::parent)
        .ok_or("invalid installed rustc path")?;
    let manifest: toml::Value =
        fs::read_to_string(root.join("lib/rustlib/multirust-channel-manifest.toml"))?.parse()?;
    if manifest.get("date").and_then(toml::Value::as_str) != Some("2024-07-22") {
        return Err("cached AVR nightly manifest has the wrong pinned date".into());
    }
    let expected = manifest
        .get("pkg")
        .and_then(|p| p.get("rustc"))
        .and_then(|p| p.get("version"))
        .and_then(toml::Value::as_str)
        .ok_or("AVR nightly rustc identity missing")?;
    let version = Command::new(&rustc).arg("--version").output()?;
    require_success(&version, "verify pinned AVR rustc")?;
    if String::from_utf8(version.stdout)?.trim() != format!("rustc {expected}") {
        return Err("cached AVR nightly compiler does not match its manifest".into());
    }
    let components = fs::read_to_string(root.join("lib/rustlib/components"))?;
    Ok(components.lines().any(|line| line == "rust-src")
        && root
            .join("lib/rustlib/src/rust/library/core/src/lib.rs")
            .is_file()
        && root
            .join("lib/rustlib/src/rust/library/alloc/src/lib.rs")
            .is_file())
}

pub(super) fn build(root: &Path) -> Result<RustFirmwareArtifact, Box<dyn std::error::Error>> {
    build_binary(root, "conduit-avr-promicro-host", ELF_NAME, HEX_NAME)
}

pub(super) fn build_receive_only(
    root: &Path,
) -> Result<RustFirmwareArtifact, Box<dyn std::error::Error>> {
    build_binary(
        root,
        RECEIVE_ONLY_BIN,
        "conduit-avr-receive-only.elf",
        "conduit-avr-receive-only.hex",
    )
}

pub(super) fn build_audible_probe(
    root: &Path,
) -> Result<RustFirmwareArtifact, Box<dyn std::error::Error>> {
    build_binary(
        root,
        AUDIBLE_PROBE_BIN,
        "conduit-avr-audible-probe.elf",
        "conduit-avr-audible-probe.hex",
    )
}

fn build_binary(
    root: &Path,
    binary: &str,
    elf_name: &str,
    hex_name: &str,
) -> Result<RustFirmwareArtifact, Box<dyn std::error::Error>> {
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
            "--bin",
            binary,
        ])
        .arg("--manifest-path")
        .arg(&manifest)
        .current_dir(root.join(FIRMWARE))
        .env("PATH", path)
        .output()?;
    require_success(&output, "Rust AVR firmware build")?;

    let firmware_target = root.join(FIRMWARE).join("target/avr-atmega32u4/release");
    let elf = firmware_target.join(elf_name);
    if !elf.is_file() {
        return Err(format!("Rust AVR build omitted {}", elf.display()).into());
    }
    let output_dir = root.join("target/avr-promicro/build");
    fs::create_dir_all(&output_dir)?;
    let hex = output_dir.join(hex_name);
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
