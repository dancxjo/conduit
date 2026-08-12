use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file},
    ConduitosArch, ConduitosError,
};

const PREFIX: &str = "CONDUIT_LOONGARCH64_ENTRY_SIGN ";
const PROFILE: &str = "qemu-loongarch64-virt-single-cpu-2g-edk2";
const FIRMWARE_VERSION: &str = "2025.02-8+deb13u1";
const FIRMWARE_URL: &str =
    "https://deb.debian.org/debian/pool/main/e/edk2/qemu-efi-loongarch64_2025.02-8+deb13u1_all.deb";
const FIRMWARE_SHA256: &str = "2a1bd3b00413af313d6105f38bda3f8156be514f5026f3ec29ba8e81b68f0633";
const QEMU_ROM_BYTES: usize = 4 * 1024 * 1024;
const QEMU_ROM_SHA256: &str = "b4c32ce95a54346c8bc180241a21ad763c5fcf43310e95aa93ed58bf0cc0864e";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct EntrySign {
    schema: String,
    status: String,
    architecture: String,
    build_id: String,
    image_id: String,
    bootloader: String,
    emulator_profile: String,
    firmware: String,
    host_id: String,
    boot_id: String,
    runtime_bases_available: bool,
    a2_machine_wake_claimed: bool,
}

#[derive(Serialize)]
struct Proof {
    schema: &'static str,
    proof_class: &'static str,
    base_commit: String,
    architecture: &'static str,
    artifact_target: &'static str,
    artifact_sha256: String,
    image_sha256: String,
    reproducible_image: bool,
    limine_version: &'static str,
    limine_archive_sha256: &'static str,
    bootloader_artifact: &'static str,
    emulator_profile: &'static str,
    qemu_version: String,
    firmware_version: &'static str,
    firmware_sha256: &'static str,
    qemu_rom_sha256: &'static str,
    firmware_role: &'static str,
    first: EntrySign,
    second: EntrySign,
    fresh_host_id: bool,
    fresh_boot_id: bool,
    runtime_bases_available: bool,
    a2_machine_wake_claimed: bool,
}

pub fn run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Loongarch64)?;
    image::execute(ConduitosArch::Loongarch64, opts)?;
    let sign = boot_once(&paths)?;
    if opts.json {
        println!("{}", serde_json::to_string(&sign).map_err(encoding)?);
    } else if !opts.quiet {
        println!(
            "{PREFIX}{}",
            serde_json::to_string(&sign).map_err(encoding)?
        );
    }
    Ok(())
}

pub fn prove(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Loongarch64)?;
    let first_image = image::execute(ConduitosArch::Loongarch64, opts)?;
    let second_image = image::execute(ConduitosArch::Loongarch64, opts)?;
    if first_image.iso_sha256 != second_image.iso_sha256 {
        return Err(refusal(
            "non-reproducible-image",
            "identical LoongArch64 inputs produced different images",
        ));
    }
    let first = boot_once(&paths)?;
    let second = boot_once(&paths)?;
    let fresh_host_id = first.host_id != second.host_id;
    let fresh_boot_id = first.boot_id != second.boot_id;
    if !fresh_host_id || !fresh_boot_id {
        return Err(refusal(
            "stale-boot-identity",
            "independent LoongArch64 boots reused HostId or BootId",
        ));
    }
    let (qemu, _) = tools(&paths)?;
    let proof = Proof {
        schema: "conduit.conduitos.loongarch64-a1-proof/v1",
        proof_class: "freestanding-loongarch64-limine-entry",
        base_commit: git_head(&paths.root)?,
        architecture: "loongarch64",
        artifact_target: super::loongarch64_a0::TARGET,
        artifact_sha256: sha256_file(&paths.kernel)?,
        image_sha256: first_image.iso_sha256,
        reproducible_image: true,
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        bootloader_artifact: "BOOTLOONGARCH64.EFI",
        emulator_profile: PROFILE,
        qemu_version: version(&qemu, &paths)?,
        firmware_version: FIRMWARE_VERSION,
        firmware_sha256: FIRMWARE_SHA256,
        qemu_rom_sha256: QEMU_ROM_SHA256,
        firmware_role: "UEFI boot mechanism/provenance only; no semantic authority",
        first,
        second,
        fresh_host_id,
        fresh_boot_id,
        runtime_bases_available: false,
        a2_machine_wake_claimed: false,
    };
    let proof_path = paths.target.join("a1-proof.json");
    fs::write(
        &proof_path,
        serde_json::to_vec_pretty(&proof).map_err(encoding)?,
    )
    .map_err(|error| refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&proof).map_err(encoding)?);
    } else if !opts.quiet {
        println!("ConduitOS LoongArch64 A1 proof: {}", proof_path.display());
    }
    Ok(())
}

fn boot_once(paths: &Paths) -> Result<EntrySign, ConduitosError> {
    let text = boot_until(paths, PREFIX)?;
    let sign = parse(&text)?;
    validate(&sign, paths)?;
    Ok(sign)
}

pub(super) fn boot_until(paths: &Paths, terminal_prefix: &str) -> Result<String, ConduitosError> {
    if !paths.limine.join("BOOTLOONGARCH64.EFI").is_file() {
        return Err(refusal(
            "missing-loongarch64-bootloader-artifact",
            "pinned BOOTLOONGARCH64.EFI is absent",
        ));
    }
    let (qemu, firmware) = tools(paths)?;
    let log = paths.target.join("loongarch64-serial.log");
    fs::write(&log, []).map_err(|error| refusal("loongarch64-boot-failed", error.to_string()))?;
    let stream = fs::OpenOptions::new()
        .append(true)
        .open(&log)
        .map_err(|error| refusal("loongarch64-boot-failed", error.to_string()))?;
    let mut child = Command::new(qemu)
        .args([
            "-M",
            "virt",
            "-cpu",
            "la464",
            "-m",
            "2G",
            "-smp",
            "1",
            "-display",
            "none",
            "-monitor",
            "none",
            "-serial",
            "stdio",
            "-net",
            "none",
            "-no-reboot",
            "-bios",
        ])
        .arg(firmware)
        .arg("-cdrom")
        .arg(&paths.iso)
        .args(["-boot", "d"])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stream.try_clone().map_err(|error| {
            refusal("loongarch64-boot-failed", error.to_string())
        })?))
        .stderr(Stdio::from(stream))
        .spawn()
        .map_err(|error| refusal("unavailable-loongarch64-emulator", error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| refusal("loongarch64-boot-failed", error.to_string()))?
        {
            return Err(refusal(
                "loongarch64-emulator-failure",
                format!("emulator exited {status} before entry Sign"),
            ));
        }
        let text = fs::read_to_string(&log).unwrap_or_default();
        if text.contains(terminal_prefix) && text.ends_with('\n') {
            child
                .kill()
                .map_err(|error| refusal("loongarch64-boot-failed", error.to_string()))?;
            child
                .wait()
                .map_err(|error| refusal("loongarch64-boot-failed", error.to_string()))?;
            return Ok(text);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(refusal(
                "absent-loongarch64-entry-sign",
                "emulator timed out",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn parse(text: &str) -> Result<EntrySign, ConduitosError> {
    let values: Vec<_> = text
        .split(PREFIX)
        .skip(1)
        .filter_map(|suffix| suffix.lines().next())
        .collect();
    if values.len() != 1 {
        return Err(refusal(
            "absent-or-duplicate-loongarch64-entry-sign",
            format!("expected one entry Sign, found {}", values.len()),
        ));
    }
    serde_json::from_str(values[0].trim_end_matches('\r'))
        .map_err(|error| refusal("malformed-loongarch64-entry-sign", error.to_string()))
}

pub(super) fn validate(sign: &EntrySign, paths: &Paths) -> Result<(), ConduitosError> {
    let commit = git_head(&paths.root)?;
    if sign.schema != "conduit.conduitos.loongarch64-entry-sign/v1"
        || sign.status != "entered"
        || sign.architecture != "loongarch64"
        || sign.build_id != commit
        || sign.image_id != format!("conduitos-image/{commit}/loongarch64/v1")
        || sign.bootloader != format!("Limine {LIMINE_VERSION}/BOOTLOONGARCH64.EFI")
        || sign.emulator_profile != PROFILE
        || sign.firmware != "EDK2 QEMU_EFI.fd (mechanism only)"
        || !sign.host_id.starts_with("host-loongarch64-")
        || !sign.boot_id.starts_with("boot-loongarch64-")
        || sign.runtime_bases_available
        || sign.a2_machine_wake_claimed
    {
        return Err(refusal(
            "stale-or-invalid-loongarch64-entry-sign",
            "entry Sign does not match exact artifact, image, Limine, firmware profile, and A1 stop line",
        ));
    }
    Ok(())
}

pub(super) fn tools(paths: &Paths) -> Result<(PathBuf, PathBuf), ConduitosError> {
    let local_qemu = paths
        .root
        .join("target/conduitos/toolchain/riscv64-root/usr/bin/qemu-system-loongarch64");
    let qemu = [
        PathBuf::from("/usr/bin/qemu-system-loongarch64"),
        local_qemu,
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or_else(|| {
        refusal(
            "unavailable-loongarch64-emulator",
            "qemu-system-loongarch64 is required",
        )
    })?;
    let firmware = prepare_firmware(paths)?;
    Ok((qemu, firmware))
}

fn prepare_firmware(paths: &Paths) -> Result<PathBuf, ConduitosError> {
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

fn version(qemu: &Path, paths: &Paths) -> Result<String, ConduitosError> {
    let output = Command::new(qemu)
        .arg("--version")
        .current_dir(&paths.root)
        .output()
        .map_err(|error| refusal("unavailable-loongarch64-emulator", error.to_string()))?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned())
}

fn reject_dry_run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        Err(refusal(
            "dry-run-has-no-entry-sign",
            "LoongArch64 A1 requires emulator execution",
        ))
    } else {
        Ok(())
    }
}

fn encoding(error: serde_json::Error) -> ConduitosError {
    refusal("proof-record-failed", error.to_string())
}

fn refusal(reason: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(reason, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_and_duplicate_entry_signs_refuse() {
        assert!(parse("").is_err());
        assert!(parse(&format!("{PREFIX}{{}}\n{PREFIX}{{}}\n")).is_err());
    }
}
