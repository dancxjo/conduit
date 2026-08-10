use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::{
    image,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file},
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use serde::{Deserialize, Serialize};

const SIGN_PREFIX: &str = "CONDUIT_IA32_ENTRY_SIGN ";
const MACHINE_PREFIX: &str = "CONDUIT_IA32_MACHINE_SIGN ";
const QEMU_PROFILE: &str = "qemu-i386-q35-single-cpu-512m-uefi-debugcon";

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
    base_count: u32,
    lane_count: u32,
    memory_arena_bytes: u32,
    timer_slots: u32,
    interrupt_fact_slots: u32,
    controller: String,
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
struct A2Proof {
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
    firmware_profile: &'static str,
    firmware: String,
    qemu_version: String,
    first_entry: EntrySign,
    second_entry: EntrySign,
    first_machine: MachineSign,
    second_machine: MachineSign,
    fresh_host_id: bool,
    fresh_boot_id: bool,
    runtime_bases_available: bool,
    a2_machine_wake_claimed: bool,
    a3_ordinary_form_claimed: bool,
}

pub fn run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Ia32)?;
    image::execute(ConduitosArch::Ia32, opts)?;
    let (sign, machine) = boot_once(&paths)?;
    if opts.json {
        println!("{}", serde_json::to_string(&sign).map_err(encoding)?);
    } else if !opts.quiet {
        println!(
            "{SIGN_PREFIX}{}",
            serde_json::to_string(&sign).map_err(encoding)?
        );
        println!(
            "{MACHINE_PREFIX}{}",
            serde_json::to_string(&machine).map_err(encoding)?
        );
    }
    Ok(())
}

pub fn prove(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Ia32)?;
    let first_image = image::execute(ConduitosArch::Ia32, opts)?;
    let second_image = image::execute(ConduitosArch::Ia32, opts)?;
    if first_image.iso_sha256 != second_image.iso_sha256 {
        return Err(refusal(
            "non-reproducible-image",
            "identical IA-32 inputs produced different images",
        ));
    }
    let (first_entry, first_machine) = boot_once(&paths)?;
    let (second_entry, second_machine) = boot_once(&paths)?;
    let fresh_host_id = first_entry.host_id != second_entry.host_id;
    let fresh_boot_id = first_entry.boot_id != second_entry.boot_id;
    if !fresh_host_id || !fresh_boot_id {
        return Err(refusal(
            "stale-boot-identity",
            "independent IA-32 boots reused HostId or BootId",
        ));
    }
    let (firmware, _) = firmware_paths(&paths)?;
    let proof = A2Proof {
        schema: "conduit.conduitos.ia32-a2-proof/v1",
        proof_class: "freestanding-ia32-production-kernel-real-pit-wake",
        base_commit: git_head(&paths.root)?,
        architecture: "ia32",
        artifact_target: "i686-freestanding-elf32",
        artifact_sha256: sha256_file(&paths.kernel)?,
        image_sha256: first_image.iso_sha256,
        reproducible_image: true,
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        bootloader_artifact: "BOOTIA32.EFI",
        firmware_profile: QEMU_PROFILE,
        firmware: firmware.display().to_string(),
        qemu_version: qemu_version(&paths)?,
        first_entry,
        second_entry,
        first_machine,
        second_machine,
        fresh_host_id,
        fresh_boot_id,
        runtime_bases_available: true,
        a2_machine_wake_claimed: true,
        a3_ordinary_form_claimed: false,
    };
    let proof_path = paths.target.join("a2-proof.json");
    fs::write(
        &proof_path,
        serde_json::to_vec_pretty(&proof).map_err(encoding)?,
    )
    .map_err(|error| refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&proof).map_err(encoding)?);
    } else if !opts.quiet {
        println!("ConduitOS IA-32 A2 proof: {}", proof_path.display());
    }
    Ok(())
}

fn reject_dry_run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        Err(refusal(
            "dry-run-has-no-entry-sign",
            "IA-32 A1 requires UEFI emulator execution",
        ))
    } else {
        Ok(())
    }
}

fn boot_once(paths: &Paths) -> Result<(EntrySign, MachineSign), ConduitosError> {
    if !paths.limine.join("BOOTIA32.EFI").is_file() {
        return Err(refusal(
            "missing-ia32-bootloader-artifact",
            "pinned BOOTIA32.EFI is absent",
        ));
    }
    let (firmware, vars_template) = firmware_paths(paths)?;
    let vars = paths.target.join("ovmf32-vars.fd");
    let transcript_path = paths.target.join("ia32-debugcon.log");
    fs::copy(vars_template, &vars)
        .map_err(|error| refusal("unavailable-ia32-firmware", error.to_string()))?;
    fs::write(&transcript_path, [])
        .map_err(|error| refusal("ia32-boot-failed", error.to_string()))?;
    let mut child = Command::new("qemu-system-i386");
    child
        .args([
            "-machine",
            "q35",
            "-cpu",
            "qemu32",
            "-m",
            "512M",
            "-smp",
            "1",
            "-display",
            "none",
            "-monitor",
            "none",
            "-serial",
            "none",
            "-net",
            "none",
            "-debugcon",
            &format!("file:{}", transcript_path.display()),
            "-global",
            "isa-debugcon.iobase=0xe9",
            "-drive",
        ])
        .arg(format!(
            "if=pflash,format=raw,readonly=on,file={}",
            firmware.display()
        ))
        .arg("-drive")
        .arg(format!("if=pflash,format=raw,file={}", vars.display()))
        .arg("-cdrom")
        .arg(&paths.iso)
        .args(["-boot", "d"])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = child
        .spawn()
        .map_err(|error| refusal("unavailable-ia32-emulator", error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| refusal("ia32-boot-failed", error.to_string()))?
        {
            let output = child
                .wait_with_output()
                .map_err(|error| refusal("ia32-boot-failed", error.to_string()))?;
            return Err(refusal(
                "ia32-emulator-failure",
                format!(
                    "emulator exited {status} before entry Sign; stderr: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        let transcript = fs::read_to_string(&transcript_path).unwrap_or_default();
        if transcript.contains(SIGN_PREFIX)
            && transcript.contains(MACHINE_PREFIX)
            && transcript.ends_with('\n')
        {
            child
                .kill()
                .map_err(|error| refusal("ia32-boot-failed", error.to_string()))?;
            child
                .wait()
                .map_err(|error| refusal("ia32-boot-failed", error.to_string()))?;
            let sign = parse_one(&transcript)?;
            validate(&sign, paths)?;
            let machine = parse_machine(&transcript, &sign)?;
            return Ok((sign, machine));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(refusal("absent-ia32-entry-sign", "emulator timed out"));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn parse_machine(transcript: &str, entry: &EntrySign) -> Result<MachineSign, ConduitosError> {
    let values: Vec<_> = transcript
        .split(MACHINE_PREFIX)
        .skip(1)
        .filter_map(|suffix| suffix.lines().next())
        .collect();
    if values.len() != 1 {
        return Err(refusal(
            "absent-ia32-machine-sign",
            format!("expected one machine Sign, found {}", values.len()),
        ));
    }
    let sign: MachineSign = serde_json::from_str(values[0])
        .map_err(|error| refusal("malformed-ia32-machine-sign", error.to_string()))?;
    validate_machine(&sign, entry)?;
    Ok(sign)
}

fn validate_machine(sign: &MachineSign, entry: &EntrySign) -> Result<(), ConduitosError> {
    if sign.schema != "conduit.conduitos.ia32-a2-sign/v1"
        || sign.status != "completed"
        || sign.architecture != "ia32"
        || sign.boot_id != entry.boot_id
        || sign.lane_id != "lane/ia32/cooperative/0"
        || sign.base_count != 7
        || sign.lane_count != 1
        || sign.memory_arena_bytes != 4096
        || sign.timer_slots != 1
        || sign.interrupt_fact_slots != 1
        || sign.controller != "8259-pic-remapped-irq0-vector32"
        || sign.wake_source != "8254-pit-channel0-irq0"
        || sign.wake_irq != 32
        || sign.idle_entries == 0
        || sign.timer_wakes != 1
        || sign.kernel_decisions == 0
        || sign.kernel_signs == 0
        || sign.pending_host_operations != 0
        || sign.sequence
            != [
                "machine-init",
                "lane-handoff",
                "idle",
                "timer-wake",
                "terminal",
            ]
        || sign.a3_ordinary_form_claimed
    {
        return Err(refusal("stale-or-invalid-ia32-machine-sign", "machine Sign does not prove exact finite Bases, sole kernel lane, real PIT wake, and terminal progress"));
    }
    Ok(())
}

fn parse_one(transcript: &str) -> Result<EntrySign, ConduitosError> {
    let values: Vec<_> = transcript
        .split(SIGN_PREFIX)
        .skip(1)
        .filter_map(|suffix| suffix.lines().next())
        .collect();
    if values.len() != 1 {
        return Err(refusal(
            "absent-ia32-entry-sign",
            format!("expected one entry Sign, found {}", values.len()),
        ));
    }
    serde_json::from_str(values[0])
        .map_err(|error| refusal("malformed-ia32-entry-sign", error.to_string()))
}

fn validate(sign: &EntrySign, paths: &Paths) -> Result<(), ConduitosError> {
    let commit = git_head(&paths.root)?;
    validate_fields(sign, &commit)
}

fn validate_fields(sign: &EntrySign, commit: &str) -> Result<(), ConduitosError> {
    if sign.schema != "conduit.conduitos.ia32-entry-sign/v1"
        || sign.status != "entered"
        || sign.architecture != "ia32"
        || sign.build_id != commit
        || sign.image_id != format!("conduitos-image/{commit}/ia32/v1")
        || sign.bootloader != format!("Limine {LIMINE_VERSION}/BOOTIA32.EFI")
        || sign.emulator_profile != QEMU_PROFILE
        || !sign.host_id.starts_with("host-ia32-")
        || !sign.boot_id.starts_with("boot-ia32-")
    {
        return Err(refusal(
            "stale-or-invalid-ia32-entry-sign",
            "entry Sign does not match exact IA-32 artifact, image, bootloader, and profile",
        ));
    }
    Ok(())
}

fn firmware_paths(paths: &Paths) -> Result<(PathBuf, PathBuf), ConduitosError> {
    [
        PathBuf::from("/usr/share/OVMF"),
        paths.root.join("target/conduitos/toolchain/ovmf-ia32"),
    ]
    .into_iter()
    .map(|root| {
        (
            root.join("OVMF32_CODE_4M.fd"),
            root.join("OVMF32_VARS_4M.fd"),
        )
    })
    .find(|(code, vars)| code.is_file() && vars.is_file())
    .ok_or_else(|| {
        refusal(
            "unavailable-ia32-firmware",
            "OVMF32_CODE_4M.fd and OVMF32_VARS_4M.fd are required",
        )
    })
}

fn qemu_version(paths: &Paths) -> Result<String, ConduitosError> {
    let output = Command::new("qemu-system-i386")
        .arg("--version")
        .current_dir(&paths.root)
        .output()
        .map_err(|error| refusal("unavailable-ia32-emulator", error.to_string()))?;
    if !output.status.success() {
        return Err(refusal(
            "unavailable-ia32-emulator",
            output.status.to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned())
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
    fn parser_rejects_absent_duplicate_and_wrong_architecture_signs() {
        assert!(parse_one("").is_err());
        let sign = EntrySign {
            schema: "conduit.conduitos.ia32-entry-sign/v1".into(),
            status: "entered".into(),
            architecture: "ia32".into(),
            build_id: "commit".into(),
            image_id: "conduitos-image/commit/ia32/v1".into(),
            bootloader: "Limine 12.5.2/BOOTIA32.EFI".into(),
            emulator_profile: QEMU_PROFILE.into(),
            host_id: "host-ia32-1".into(),
            boot_id: "boot-ia32-1".into(),
        };
        let encoded = serde_json::to_string(&sign).unwrap();
        assert!(parse_one(&format!("{SIGN_PREFIX}{encoded}\n{SIGN_PREFIX}{encoded}\n")).is_err());
        assert!(validate_fields(&sign, "commit").is_ok());
        let mut wrong = sign;
        wrong.architecture = "x86_64".into();
        assert!(validate_fields(&wrong, "commit").is_err());
        wrong.architecture = "ia32".into();
        wrong.emulator_profile = "stale-profile".into();
        assert!(validate_fields(&wrong, "commit").is_err());
    }

    #[test]
    fn machine_sign_rejects_wrong_wake_controller_and_duplicate_lane() {
        let entry = EntrySign {
            schema: "conduit.conduitos.ia32-entry-sign/v1".into(),
            status: "entered".into(),
            architecture: "ia32".into(),
            build_id: "commit".into(),
            image_id: "conduitos-image/commit/ia32/v1".into(),
            bootloader: "Limine 12.5.2/BOOTIA32.EFI".into(),
            emulator_profile: QEMU_PROFILE.into(),
            host_id: "host-ia32-1".into(),
            boot_id: "boot-ia32-1".into(),
        };
        let mut machine = MachineSign {
            schema: "conduit.conduitos.ia32-a2-sign/v1".into(),
            status: "completed".into(),
            architecture: "ia32".into(),
            boot_id: entry.boot_id.clone(),
            lane_id: "lane/ia32/cooperative/0".into(),
            base_count: 7,
            lane_count: 1,
            memory_arena_bytes: 4096,
            timer_slots: 1,
            interrupt_fact_slots: 1,
            controller: "8259-pic-remapped-irq0-vector32".into(),
            wake_source: "8254-pit-channel0-irq0".into(),
            wake_irq: 32,
            idle_entries: 1,
            timer_wakes: 1,
            kernel_decisions: 2,
            kernel_signs: 5,
            pending_host_operations: 0,
            sequence: [
                "machine-init",
                "lane-handoff",
                "idle",
                "timer-wake",
                "terminal",
            ]
            .map(String::from)
            .into(),
            a3_ordinary_form_claimed: false,
        };
        assert!(validate_machine(&machine, &entry).is_ok());
        machine.wake_source = "synthetic".into();
        assert!(validate_machine(&machine, &entry).is_err());
        machine.wake_source = "8254-pit-channel0-irq0".into();
        machine.controller = "stale-pic".into();
        assert!(validate_machine(&machine, &entry).is_err());
        machine.controller = "8259-pic-remapped-irq0-vector32".into();
        machine.lane_count = 2;
        assert!(validate_machine(&machine, &entry).is_err());
    }
}
