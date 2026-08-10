use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, AARCH64_QEMU_PROFILE, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file},
    ConduitosArch, ConduitosError,
};

const FIRMWARE: &str = "/usr/share/qemu-efi-aarch64/QEMU_EFI.fd";
const SIGN_PREFIX: &str = "CONDUIT_AARCH64_ENTRY_SIGN ";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EntrySign {
    schema: String,
    status: String,
    architecture: String,
    build_id: String,
    image_id: String,
    bootloader: String,
    emulator_profile: String,
    host_id: String,
    boot_id: String,
}

#[derive(Serialize)]
struct A1Proof {
    schema: &'static str,
    proof_class: &'static str,
    base_commit: String,
    architecture: &'static str,
    rust_target: &'static str,
    limine_version: &'static str,
    limine_archive_sha256: &'static str,
    limine_efi_artifact: &'static str,
    emulator_profile: &'static str,
    firmware: &'static str,
    qemu_version: String,
    iso_sha256: String,
    reproducible_image: bool,
    first_entry: EntrySign,
    second_entry: EntrySign,
    fresh_host_id: bool,
    fresh_boot_id: bool,
    a2_machine_wake_claimed: bool,
}

pub fn run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let _ = image::execute(ConduitosArch::Aarch64, opts)?;
    let sign = boot_once(&paths)?;
    if opts.json {
        println!("{}", serde_json::to_string(&sign).map_err(encoding)?);
    } else if !opts.quiet {
        println!(
            "{SIGN_PREFIX}{}",
            serde_json::to_string(&sign).map_err(encoding)?
        );
    }
    Ok(())
}

pub fn prove(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-entry-sign",
            "AArch64 A1 proof requires actual emulator execution",
        ));
    }
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let first_image = image::execute(ConduitosArch::Aarch64, opts)?;
    let second_image = image::execute(ConduitosArch::Aarch64, opts)?;
    if first_image.iso_sha256 != second_image.iso_sha256 {
        return Err(ConduitosError::refusal(
            "non-reproducible-image",
            "identical AArch64 inputs produced different images",
        ));
    }
    let first_entry = boot_once(&paths)?;
    let second_entry = boot_once(&paths)?;
    let fresh_host_id = first_entry.host_id != second_entry.host_id;
    let fresh_boot_id = first_entry.boot_id != second_entry.boot_id;
    if !fresh_host_id || !fresh_boot_id {
        return Err(ConduitosError::refusal(
            "stale-boot-identity",
            "independent AArch64 boots reused HostId or BootId",
        ));
    }
    let proof = A1Proof {
        schema: "conduit.conduitos.aarch64-a1-proof/v1",
        proof_class: "freestanding-emulator-entry-only",
        base_commit: git_head(&paths.root)?,
        architecture: "aarch64",
        rust_target: super::aarch64_a0::TARGET,
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        limine_efi_artifact: "BOOTAA64.EFI",
        emulator_profile: AARCH64_QEMU_PROFILE,
        firmware: FIRMWARE,
        qemu_version: qemu_version(&paths)?,
        iso_sha256: sha256_file(&paths.iso)?,
        reproducible_image: true,
        first_entry,
        second_entry,
        fresh_host_id,
        fresh_boot_id,
        a2_machine_wake_claimed: false,
    };
    let bytes = serde_json::to_vec_pretty(&proof).map_err(encoding)?;
    fs::write(paths.target.join("a1-proof.json"), bytes)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&proof).map_err(encoding)?);
    } else if !opts.quiet {
        println!(
            "ConduitOS AArch64 A1 proof: {}",
            paths.target.join("a1-proof.json").display()
        );
    }
    Ok(())
}

fn boot_once(paths: &Paths) -> Result<EntrySign, ConduitosError> {
    if !paths.limine.join("BOOTAA64.EFI").is_file() {
        return Err(ConduitosError::refusal(
            "missing-limine-artifact",
            "BOOTAA64.EFI is absent",
        ));
    }
    if !std::path::Path::new(FIRMWARE).is_file() {
        return Err(ConduitosError::refusal(
            "unavailable-aarch64-firmware",
            format!("required repository profile firmware is absent: {FIRMWARE}"),
        ));
    }
    let mut child = Command::new("qemu-system-aarch64")
        .args([
            "-M",
            "virt",
            "-cpu",
            "cortex-a72",
            "-m",
            "64M",
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
            "-semihosting-config",
            "enable=on,target=native",
            "-bios",
            FIRMWARE,
            "-cdrom",
            paths.iso.to_str().unwrap(),
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            ConduitosError::refusal("unavailable-aarch64-emulator", error.to_string())
        })?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match child
            .try_wait()
            .map_err(|error| ConduitosError::refusal("aarch64-boot-failed", error.to_string()))?
        {
            Some(_) => break,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ConduitosError::refusal(
                    "absent-aarch64-entry-sign",
                    "bounded QEMU profile timed out",
                ));
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|error| ConduitosError::refusal("aarch64-boot-failed", error.to_string()))?;
    let mut transcript = output.stdout;
    transcript.extend_from_slice(&output.stderr);
    let serial = String::from_utf8(transcript).map_err(|error| {
        ConduitosError::refusal("malformed-aarch64-entry-sign", error.to_string())
    })?;
    let signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix(SIGN_PREFIX))
        .collect();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "absent-aarch64-entry-sign",
            format!(
                "expected one entry Sign, found {}; stderr: {}",
                signs.len(),
                serial.trim()
            ),
        ));
    }
    let sign: EntrySign = serde_json::from_str(signs[0]).map_err(|error| {
        ConduitosError::refusal("malformed-aarch64-entry-sign", error.to_string())
    })?;
    validate(&sign, paths)?;
    Ok(sign)
}

fn validate(sign: &EntrySign, paths: &Paths) -> Result<(), ConduitosError> {
    let commit = git_head(&paths.root)?;
    let build_id = format!("conduitos-build/{commit}/aarch64/v1");
    let image_id = format!("conduitos-image/{commit}/aarch64/v1");
    if sign.schema != "conduit.conduitos.aarch64-entry-sign/v1"
        || sign.status != "entered"
        || sign.architecture != "aarch64"
        || sign.build_id != build_id
        || sign.image_id != image_id
        || sign.bootloader != "Limine 12.5.2/BOOTAA64.EFI"
        || sign.emulator_profile != AARCH64_QEMU_PROFILE
        || !sign.host_id.starts_with("host-aarch64-")
        || !sign.boot_id.starts_with("boot-aarch64-")
    {
        return Err(ConduitosError::refusal(
            "stale-aarch64-entry-sign",
            "entry Sign does not match exact artifact/profile identity",
        ));
    }
    Ok(())
}

fn qemu_version(paths: &Paths) -> Result<String, ConduitosError> {
    let output = super::profile::command(
        "qemu-system-aarch64",
        &["--version"],
        &paths.root,
        "unavailable-aarch64-emulator",
    )?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("unknown")
        .to_owned())
}

fn encoding(error: serde_json::Error) -> ConduitosError {
    ConduitosError::refusal("aarch64-proof-encoding-failed", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_x86_alias_and_stale_image_identity() {
        let paths = Paths::new(ConduitosArch::Aarch64).unwrap();
        let mut sign = EntrySign {
            schema: "conduit.conduitos.aarch64-entry-sign/v1".into(),
            status: "entered".into(),
            architecture: "x86_64".into(),
            build_id: String::new(),
            image_id: String::new(),
            bootloader: "Limine 12.5.2/BOOTX64.EFI".into(),
            emulator_profile: AARCH64_QEMU_PROFILE.into(),
            host_id: "host-aarch64-1".into(),
            boot_id: "boot-aarch64-1".into(),
        };
        assert!(validate(&sign, &paths).is_err());
        sign.architecture = "aarch64".into();
        assert!(validate(&sign, &paths).is_err());
    }
}
