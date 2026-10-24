//! Existing reviewed LoongArch ROM acquisition, separated from execution selection.
use super::{profile::Paths, report::sha256_file, ConduitosError};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
pub(super) const FIRMWARE_VERSION: &str = "2025.02-8+deb13u1";
const FIRMWARE_URL: &str =
    "https://deb.debian.org/debian/pool/main/e/edk2/qemu-efi-loongarch64_2025.02-8+deb13u1_all.deb";
pub(super) const FIRMWARE_SHA256: &str =
    "2a1bd3b00413af313d6105f38bda3f8156be514f5026f3ec29ba8e81b68f0633";
const QEMU_ROM_BYTES: usize = 4 * 1024 * 1024;
pub(super) const QEMU_ROM_SHA256: &str =
    "b4c32ce95a54346c8bc180241a21ad763c5fcf43310e95aa93ed58bf0cc0864e";

pub(super) fn paths(paths: &Paths) -> Result<PathBuf, ConduitosError> {
    let rom = paths
        .root
        .join("target/conduitos/toolchain/QEMU_EFI.loongarch64-qemu8.fd");
    if fs::metadata(&rom).is_ok_and(|m| m.len() == QEMU_ROM_BYTES as u64)
        && sha256_file(&rom)? == QEMU_ROM_SHA256
    {
        return Ok(rom);
    }
    Err(refusal(
        "unavailable-loongarch64-firmware",
        "run `cargo xtask make conduitos prepare-loongarch64-domain-emulator --graphical`",
    ))
}

pub(super) fn prepare(paths: &Paths) -> Result<PathBuf, ConduitosError> {
    let toolchain = paths.root.join("target/conduitos/toolchain");
    let package = toolchain.join("qemu-efi-loongarch64.deb");
    let root = toolchain.join("loongarch64-root");
    let packaged_firmware = root.join("usr/share/qemu-efi-loongarch64/QEMU_EFI.fd");
    let firmware = toolchain.join("QEMU_EFI.loongarch64-qemu8.fd");
    fs::create_dir_all(&toolchain)
        .map_err(|error| refusal("unavailable-loongarch64-firmware", error.to_string()))?;
    if !package.is_file() {
        command(
            "curl",
            &[
                "--fail",
                "--location",
                "--remove-on-error",
                "--retry",
                "3",
                "--retry-all-errors",
                "--retry-delay",
                "1",
                "--output",
                package.to_str().unwrap(),
                FIRMWARE_URL,
            ],
            &paths.root,
        )?;
    }
    let digest = sha256_file(&package)?;
    if digest != FIRMWARE_SHA256 {
        return Err(refusal(
            "stale-loongarch64-firmware",
            format!("firmware package digest {digest} does not match pinned {FIRMWARE_SHA256}"),
        ));
    }
    if !packaged_firmware.is_file() {
        fs::create_dir_all(&root)
            .map_err(|error| refusal("unavailable-loongarch64-firmware", error.to_string()))?;
        command(
            "dpkg-deb",
            &[
                "--extract",
                package.to_str().unwrap(),
                root.to_str().unwrap(),
            ],
            &paths.root,
        )?;
    }
    if !firmware.is_file() {
        let bytes = fs::read(&packaged_firmware)
            .map_err(|error| refusal("unavailable-loongarch64-firmware", error.to_string()))?;
        if bytes.len() < QEMU_ROM_BYTES {
            return Err(refusal(
                "stale-loongarch64-firmware",
                format!("packaged firmware is only {} bytes", bytes.len()),
            ));
        }
        fs::write(&firmware, &bytes[..QEMU_ROM_BYTES])
            .map_err(|error| refusal("unavailable-loongarch64-firmware", error.to_string()))?;
    }
    let rom_digest = sha256_file(&firmware)?;
    if rom_digest != QEMU_ROM_SHA256 {
        return Err(refusal(
            "stale-loongarch64-firmware",
            format!("QEMU ROM digest {rom_digest} does not match pinned {QEMU_ROM_SHA256}"),
        ));
    }
    Ok(firmware)
}

fn command(program: &str, args: &[&str], cwd: &Path) -> Result<(), ConduitosError> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| refusal("unavailable-loongarch64-firmware", error.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(refusal(
            "unavailable-loongarch64-firmware",
            format!(
                "{program} exited {}; stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ))
    }
}

fn refusal(code: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(code, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn execution_refuses_missing_or_corrupt_cache_without_acquisition_or_replacement() {
        let root = std::env::temp_dir().join(format!(
            "conduit-loong-firmware-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut selected = Paths::new(super::super::ConduitosArch::Loongarch64).unwrap();
        selected.root = root.clone();
        assert!(paths(&selected).is_err());
        assert!(!root.exists());
        let rom = root.join("target/conduitos/toolchain/QEMU_EFI.loongarch64-qemu8.fd");
        fs::create_dir_all(rom.parent().unwrap()).unwrap();
        fs::File::create(&rom)
            .unwrap()
            .set_len(QEMU_ROM_BYTES as u64)
            .unwrap();
        let before = sha256_file(&rom).unwrap();
        assert!(paths(&selected).is_err());
        assert_eq!(sha256_file(&rom).unwrap(), before);
        assert_eq!(fs::read_dir(rom.parent().unwrap()).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
