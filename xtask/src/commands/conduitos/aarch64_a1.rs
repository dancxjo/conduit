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
const MACHINE_SIGN_PREFIX: &str = "CONDUIT_AARCH64_MACHINE_SIGN ";

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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MachineSign {
    schema: String,
    status: String,
    architecture: String,
    boot_id: String,
    lane_id: String,
    lane_count: u32,
    timer_slots: u32,
    interrupt_fact_slots: u32,
    wake_source: String,
    wake_irq: u32,
    idle_entries: u32,
    timer_wakes: u32,
    kernel_decisions: u32,
    kernel_signs: u32,
    pending_host_operations: u32,
    sequence: Vec<String>,
    a3_ordinary_form_claimed: bool,
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
    firmware: String,
    qemu_version: String,
    iso_sha256: String,
    reproducible_image: bool,
    first_entry: EntrySign,
    second_entry: EntrySign,
    fresh_host_id: bool,
    fresh_boot_id: bool,
    first_machine: MachineSign,
    second_machine: MachineSign,
    a2_machine_wake_claimed: bool,
}

pub fn run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let _ = image::execute(ConduitosArch::Aarch64, opts)?;
    let (entry, machine) = boot_once(&paths)?;
    if opts.json {
        println!("{}", serde_json::to_string(&machine).map_err(encoding)?);
    } else if !opts.quiet {
        println!(
            "{SIGN_PREFIX}{}",
            serde_json::to_string(&entry).map_err(encoding)?
        );
        println!(
            "{MACHINE_SIGN_PREFIX}{}",
            serde_json::to_string(&machine).map_err(encoding)?
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
    let (first_entry, first_machine) = boot_once(&paths)?;
    let (second_entry, second_machine) = boot_once(&paths)?;
    let fresh_host_id = first_entry.host_id != second_entry.host_id;
    let fresh_boot_id = first_entry.boot_id != second_entry.boot_id;
    if !fresh_host_id || !fresh_boot_id {
        return Err(ConduitosError::refusal(
            "stale-boot-identity",
            "independent AArch64 boots reused HostId or BootId",
        ));
    }
    let proof = A1Proof {
        schema: "conduit.conduitos.aarch64-a2-proof/v1",
        proof_class: "freestanding-emulator-machine-wake",
        base_commit: git_head(&paths.root)?,
        architecture: "aarch64",
        rust_target: super::aarch64_a0::TARGET,
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        limine_efi_artifact: "BOOTAA64.EFI",
        emulator_profile: AARCH64_QEMU_PROFILE,
        firmware: firmware_path().to_string_lossy().into_owned(),
        qemu_version: qemu_version(&paths)?,
        iso_sha256: sha256_file(&paths.iso)?,
        reproducible_image: true,
        first_entry,
        second_entry,
        first_machine,
        second_machine,
        fresh_host_id,
        fresh_boot_id,
        a2_machine_wake_claimed: true,
    };
    let bytes = serde_json::to_vec_pretty(&proof).map_err(encoding)?;
    fs::write(paths.target.join("a2-proof.json"), bytes)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&proof).map_err(encoding)?);
    } else if !opts.quiet {
        println!(
            "ConduitOS AArch64 A2 proof: {}",
            paths.target.join("a2-proof.json").display()
        );
    }
    Ok(())
}

fn boot_once(paths: &Paths) -> Result<(EntrySign, MachineSign), ConduitosError> {
    if !paths.limine.join("BOOTAA64.EFI").is_file() {
        return Err(ConduitosError::refusal(
            "missing-limine-artifact",
            "BOOTAA64.EFI is absent",
        ));
    }
    let firmware = firmware_path();
    if !firmware.is_file() {
        return Err(ConduitosError::refusal(
            "unavailable-aarch64-firmware",
            format!(
                "required repository profile firmware is absent: {}",
                firmware.display()
            ),
        ));
    }
    let mut child = Command::new("qemu-system-aarch64")
        .args([
            "-M",
            "virt",
            "-cpu",
            "cortex-a72",
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
            "-no-reboot",
            "-semihosting-config",
            "enable=on,target=native",
            "-bios",
            firmware.to_str().ok_or_else(|| {
                ConduitosError::refusal(
                    "unavailable-aarch64-firmware",
                    "firmware path is not UTF-8",
                )
            })?,
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
                let output = child.wait_with_output().map_err(|error| {
                    ConduitosError::refusal("aarch64-boot-failed", error.to_string())
                })?;
                return Err(ConduitosError::refusal(
                    "absent-aarch64-entry-sign",
                    format!(
                        "bounded QEMU profile timed out; stdout: {}; stderr: {}",
                        String::from_utf8_lossy(&output.stdout).trim(),
                        String::from_utf8_lossy(&output.stderr).trim()
                    ),
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
        .split(SIGN_PREFIX)
        .skip(1)
        .filter_map(|suffix| suffix.lines().next())
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
    let machine_signs: Vec<_> = serial
        .split(MACHINE_SIGN_PREFIX)
        .skip(1)
        .filter_map(|suffix| suffix.lines().next())
        .collect();
    if machine_signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "absent-aarch64-machine-sign",
            format!(
                "expected one A2 machine Sign, found {}",
                machine_signs.len()
            ),
        ));
    }
    let machine: MachineSign = serde_json::from_str(machine_signs[0]).map_err(|error| {
        ConduitosError::refusal("malformed-aarch64-machine-sign", error.to_string())
    })?;
    validate_machine(&machine, &sign)?;
    Ok((sign, machine))
}

fn validate_machine(machine: &MachineSign, entry: &EntrySign) -> Result<(), ConduitosError> {
    let expected_sequence = [
        "machine-init",
        "lane-handoff",
        "idle",
        "timer-wake",
        "terminal",
    ];
    if machine.schema != "conduit.conduitos.aarch64-a2-sign/v1"
        || machine.status != "completed"
        || machine.architecture != "aarch64"
        || machine.boot_id != entry.boot_id
        || machine.lane_id != "lane/aarch64/cooperative/0"
        || machine.lane_count != 1
        || machine.timer_slots != 1
        || machine.interrupt_fact_slots != 1
        || machine.wake_source != "arm-generic-virtual-timer-ppi-27"
        || machine.wake_irq != 27
        || machine.idle_entries == 0
        || machine.timer_wakes != 1
        || machine.kernel_decisions == 0
        || machine.kernel_signs == 0
        || machine.pending_host_operations != 0
        || machine
            .sequence
            .iter()
            .map(String::as_str)
            .ne(expected_sequence)
        || machine.a3_ordinary_form_claimed
    {
        return Err(ConduitosError::refusal(
            "stale-or-synthetic-aarch64-machine-sign",
            "A2 Sign does not prove the exact admitted lane, interruptible idle, PPI 27 wake, and terminal kernel progress",
        ));
    }
    Ok(())
}

fn firmware_path() -> std::path::PathBuf {
    std::env::var_os("CONDUITOS_AARCH64_UEFI_FIRMWARE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| FIRMWARE.into())
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

    #[test]
    fn rejects_wrong_synthetic_or_stale_machine_wakes() {
        let entry = EntrySign {
            schema: "conduit.conduitos.aarch64-entry-sign/v1".into(),
            status: "entered".into(),
            architecture: "aarch64".into(),
            build_id: "build".into(),
            image_id: "image".into(),
            bootloader: "Limine 12.5.2/BOOTAA64.EFI".into(),
            emulator_profile: AARCH64_QEMU_PROFILE.into(),
            host_id: "host-aarch64-1".into(),
            boot_id: "boot-aarch64-1".into(),
        };
        let mut machine = MachineSign {
            schema: "conduit.conduitos.aarch64-a2-sign/v1".into(),
            status: "completed".into(),
            architecture: "aarch64".into(),
            boot_id: entry.boot_id.clone(),
            lane_id: "lane/aarch64/cooperative/0".into(),
            lane_count: 1,
            timer_slots: 1,
            interrupt_fact_slots: 1,
            wake_source: "synthetic-poll".into(),
            wake_irq: 27,
            idle_entries: 1,
            timer_wakes: 1,
            kernel_decisions: 2,
            kernel_signs: 2,
            pending_host_operations: 0,
            sequence: [
                "machine-init",
                "lane-handoff",
                "idle",
                "timer-wake",
                "terminal",
            ]
            .map(str::to_owned)
            .into(),
            a3_ordinary_form_claimed: false,
        };
        assert!(validate_machine(&machine, &entry).is_err());
        machine.wake_source = "arm-generic-virtual-timer-ppi-27".into();
        machine.wake_irq = 30;
        assert!(validate_machine(&machine, &entry).is_err());
        machine.wake_irq = 27;
        machine.boot_id = "boot-aarch64-stale".into();
        assert!(validate_machine(&machine, &entry).is_err());
    }
}
