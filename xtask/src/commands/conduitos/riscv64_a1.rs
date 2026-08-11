use super::{
    image,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file},
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const PREFIX: &str = "CONDUIT_RISCV64_ENTRY_SIGN ";
const PROFILE: &str = "qemu-riscv64-virt-single-hart-256m-opensbi-uboot";

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
    opensbi: String,
    uboot: String,
    first: EntrySign,
    second: EntrySign,
    fresh_host_id: bool,
    fresh_boot_id: bool,
    runtime_bases_available: bool,
    a2_machine_wake_claimed: bool,
}

pub fn run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    image::execute(ConduitosArch::Riscv64, opts)?;
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
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    let image1 = image::execute(ConduitosArch::Riscv64, opts)?;
    let image2 = image::execute(ConduitosArch::Riscv64, opts)?;
    if image1.iso_sha256 != image2.iso_sha256 {
        return Err(refusal(
            "non-reproducible-image",
            "identical RISC-V64 inputs produced different images",
        ));
    }
    let first = boot_once(&paths)?;
    let second = boot_once(&paths)?;
    let fresh_host_id = first.host_id != second.host_id;
    let fresh_boot_id = first.boot_id != second.boot_id;
    if !fresh_host_id || !fresh_boot_id {
        return Err(refusal(
            "stale-boot-identity",
            "independent RISC-V64 boots reused HostId or BootId",
        ));
    }
    let (qemu, opensbi, uboot) = tools(&paths)?;
    let proof = Proof {
        schema: "conduit.conduitos.riscv64-a1-proof/v1",
        proof_class: "freestanding-riscv64-limine-entry",
        base_commit: git_head(&paths.root)?,
        architecture: "riscv64",
        artifact_target: super::riscv64_a0::TARGET,
        artifact_sha256: sha256_file(&paths.kernel)?,
        image_sha256: image1.iso_sha256,
        reproducible_image: true,
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        bootloader_artifact: "BOOTRISCV64.EFI",
        emulator_profile: PROFILE,
        qemu_version: version(&qemu, &paths)?,
        opensbi: opensbi.display().to_string(),
        uboot: uboot.display().to_string(),
        first,
        second,
        fresh_host_id,
        fresh_boot_id,
        runtime_bases_available: false,
        a2_machine_wake_claimed: false,
    };
    let path = paths.target.join("a1-proof.json");
    fs::write(&path, serde_json::to_vec_pretty(&proof).map_err(encoding)?)
        .map_err(|e| refusal("proof-record-failed", e.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&proof).map_err(encoding)?);
    } else if !opts.quiet {
        println!("ConduitOS RISC-V64 A1 proof: {}", path.display());
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
    if !paths.limine.join("BOOTRISCV64.EFI").is_file() {
        return Err(refusal(
            "missing-riscv64-bootloader-artifact",
            "pinned BOOTRISCV64.EFI is absent",
        ));
    }
    let (qemu, opensbi, uboot) = tools(paths)?;
    let log = paths.target.join("riscv64-serial.log");
    fs::write(&log, []).map_err(|e| refusal("riscv64-boot-failed", e.to_string()))?;
    let stream = fs::OpenOptions::new()
        .append(true)
        .open(&log)
        .map_err(|e| refusal("riscv64-boot-failed", e.to_string()))?;
    let mut command = Command::new(qemu);
    command
        .args([
            "-machine",
            "virt",
            "-cpu",
            "rv64,sv57=off,sv48=off",
            "-m",
            "256M",
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
            "-bios",
        ])
        .arg(opensbi)
        .arg("-kernel")
        .arg(uboot)
        .arg("-drive")
        .arg(format!(
            "if=virtio,format=raw,readonly=on,file={}",
            paths.iso.display()
        ))
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            stream
                .try_clone()
                .map_err(|e| refusal("riscv64-boot-failed", e.to_string()))?,
        ))
        .stderr(Stdio::from(stream));
    let mut child = command
        .spawn()
        .map_err(|e| refusal("unavailable-riscv64-emulator", e.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| refusal("riscv64-boot-failed", e.to_string()))?
        {
            return Err(refusal(
                "riscv64-emulator-failure",
                format!("emulator exited {status} before entry Sign"),
            ));
        }
        let text = fs::read_to_string(&log).unwrap_or_default();
        if text.contains(terminal_prefix) && text.ends_with('\n') {
            child
                .kill()
                .map_err(|e| refusal("riscv64-boot-failed", e.to_string()))?;
            child
                .wait()
                .map_err(|e| refusal("riscv64-boot-failed", e.to_string()))?;
            return Ok(text);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(refusal("absent-riscv64-entry-sign", "emulator timed out"));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn parse(text: &str) -> Result<EntrySign, ConduitosError> {
    let values: Vec<_> = text
        .split(PREFIX)
        .skip(1)
        .filter_map(|s| s.lines().next())
        .collect();
    if values.len() != 1 {
        return Err(refusal(
            "absent-or-duplicate-riscv64-entry-sign",
            format!("expected one entry Sign, found {}", values.len()),
        ));
    }
    serde_json::from_str(values[0].trim_end_matches('\r'))
        .map_err(|e| refusal("malformed-riscv64-entry-sign", e.to_string()))
}

pub(super) fn validate(sign: &EntrySign, paths: &Paths) -> Result<(), ConduitosError> {
    let commit = git_head(&paths.root)?;
    if sign.schema != "conduit.conduitos.riscv64-entry-sign/v1"
        || sign.status != "entered"
        || sign.architecture != "riscv64"
        || sign.build_id != commit
        || sign.image_id != format!("conduitos-image/{commit}/riscv64/v1")
        || sign.bootloader != format!("Limine {LIMINE_VERSION}/BOOTRISCV64.EFI")
        || sign.emulator_profile != PROFILE
        || sign.firmware != "OpenSBI+U-Boot EFI"
        || !sign.host_id.starts_with("host-riscv64-")
        || !sign.boot_id.starts_with("boot-riscv64-")
        || sign.runtime_bases_available
        || sign.a2_machine_wake_claimed
    {
        return Err(refusal("stale-or-invalid-riscv64-entry-sign", "entry Sign does not match exact artifact, image, Limine, firmware profile, and A1 stop line"));
    }
    Ok(())
}

pub(super) fn tools(paths: &Paths) -> Result<(PathBuf, PathBuf, PathBuf), ConduitosError> {
    let local = paths.root.join("target/conduitos/toolchain/riscv64-root");
    let qemu = [
        PathBuf::from("/usr/bin/qemu-system-riscv64"),
        local.join("usr/bin/qemu-system-riscv64"),
    ]
    .into_iter()
    .find(|p| p.is_file());
    let opensbi = [
        PathBuf::from("/usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin"),
        local.join("usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin"),
    ]
    .into_iter()
    .find(|p| p.is_file());
    let uboot = [
        PathBuf::from("/usr/lib/u-boot/qemu-riscv64_smode/uboot.elf"),
        local.join("usr/lib/u-boot/qemu-riscv64_smode/uboot.elf"),
    ]
    .into_iter()
    .find(|p| p.is_file());
    match (qemu, opensbi, uboot) {
        (Some(q), Some(o), Some(u)) => Ok((q, o, u)),
        _ => Err(refusal(
            "unavailable-riscv64-firmware",
            "qemu-system-riscv64, OpenSBI fw_dynamic, and qemu-riscv64_smode U-Boot are required",
        )),
    }
}
fn version(qemu: &PathBuf, paths: &Paths) -> Result<String, ConduitosError> {
    let out = Command::new(qemu)
        .arg("--version")
        .current_dir(&paths.root)
        .output()
        .map_err(|e| refusal("unavailable-riscv64-emulator", e.to_string()))?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .into())
}
fn reject_dry_run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        Err(refusal(
            "dry-run-has-no-entry-sign",
            "RISC-V64 A1 requires emulator execution",
        ))
    } else {
        Ok(())
    }
}
fn encoding(e: serde_json::Error) -> ConduitosError {
    refusal("proof-record-failed", e.to_string())
}
fn refusal(reason: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(reason, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_sign_refuses() {
        assert!(parse("").is_err());
    }
}
