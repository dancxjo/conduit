use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    avr_toolchain::{avr_gcc_bin, provision, verify_cores},
    metric, require_success,
};

pub(super) use super::rust_toolchain::{provision_rust, RUST_TOOLCHAIN};
pub(super) use super::setup::{AVR_HAL_REVISION, FIRMWARE};
const ELF_NAME: &str = "conduit-avr-promicro-host.elf";
const HEX_NAME: &str = "conduit-avr-promicro-host.hex";
const RECEIVE_ONLY_BIN: &str = "conduit-avr-receive-only";
const AUDIBLE_PROBE_BIN: &str = "conduit-avr-audible-probe";

pub(super) struct RustFirmwareArtifact {
    pub(super) hex: PathBuf,
    pub(super) flash_bytes: u64,
    pub(super) sram_bytes: u64,
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
    let firmware_dir = root.join(FIRMWARE);
    let manifest = firmware_dir.join("Cargo.toml");
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
        .current_dir(&firmware_dir)
        .env("PATH", path)
        .output()?;
    require_success(&output, "Rust AVR firmware build")?;

    let target_dir = env::var_os("CARGO_TARGET_DIR");
    let elf = compiled_elf_path(
        &firmware_dir,
        target_dir.as_deref().map(Path::new),
        elf_name,
    );
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

fn compiled_elf_path(firmware_dir: &Path, target_dir: Option<&Path>, elf_name: &str) -> PathBuf {
    let target_dir = target_dir.unwrap_or_else(|| Path::new("target"));
    let target_dir = if target_dir.is_absolute() {
        target_dir.to_path_buf()
    } else {
        firmware_dir.join(target_dir)
    };
    target_dir.join("avr-atmega32u4/release").join(elf_name)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::compiled_elf_path;

    #[test]
    fn compiled_elf_follows_cargo_target_dir_from_firmware_working_directory() {
        let firmware_dir = Path::new("/repo/targets/avr/firmware/promicro-host");
        let elf = "conduit-avr-promicro-host.elf";
        assert_eq!(
            compiled_elf_path(firmware_dir, None, elf),
            firmware_dir.join("target/avr-atmega32u4/release").join(elf)
        );
        assert_eq!(
            compiled_elf_path(firmware_dir, Some(Path::new("shared-target")), elf),
            firmware_dir
                .join("shared-target/avr-atmega32u4/release")
                .join(elf)
        );
        assert_eq!(
            compiled_elf_path(firmware_dir, Some(Path::new("/cache/cargo-target")), elf),
            Path::new("/cache/cargo-target/avr-atmega32u4/release").join(elf)
        );
    }
}
