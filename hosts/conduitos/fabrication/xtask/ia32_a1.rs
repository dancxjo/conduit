use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use super::{
    image,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file, GuestKernelSign},
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;

const ENTRY_PREFIX: &str = "CONDUIT_IA32_ENTRY_SIGN ";
const KERNEL_PREFIX: &str = "CONDUIT_KERNEL_SIGN ";
const IDENTITY_PREFIX: &str = "CONDUIT_IA32_A3_IDENTITY ";
const OBSERVATORY_PREFIX: &str = "CONDUIT_OBSERVATORY_SNAPSHOT ";
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
struct IdentitySign {
    image_id: String,
    wake_source: String,
    wake_irq: u32,
    a3_ordinary_form_claimed: bool,
    a4_observatory_patchbay_claimed: bool,
}

#[derive(Clone, Debug)]
struct A3Run {
    entry: EntrySign,
    kernel: GuestKernelSign,
    identity: IdentitySign,
    observatory: conduit_observatory::ObservatorySnapshot,
}

#[derive(Serialize)]
struct A3Proof {
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
    first: ProofRun,
    second: ProofRun,
    fresh_entry_host_id: bool,
    fresh_entry_boot_id: bool,
    fresh_kernel_host_id: bool,
    fresh_kernel_boot_id: bool,
    stable_semantic_identities: bool,
    fresh_realization_identities: bool,
    a3_ordinary_form_claimed: bool,
    a4_observatory_patchbay_claimed: bool,
    native_patchbay_consumed: bool,
}

#[derive(Serialize)]
struct ProofRun {
    entry: EntrySign,
    kernel: GuestKernelSign,
    identity: IdentitySign,
}

pub fn run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Ia32)?;
    image::execute(ConduitosArch::Ia32, opts)?;
    let run = boot_once(&paths)?;
    if opts.json {
        println!("{}", serde_json::to_string(&run.kernel).map_err(encoding)?);
    } else if !opts.quiet {
        println!(
            "{ENTRY_PREFIX}{}",
            serde_json::to_string(&run.entry).map_err(encoding)?
        );
        println!(
            "{KERNEL_PREFIX}{}",
            serde_json::to_string(&run.kernel).map_err(encoding)?
        );
        println!(
            "{IDENTITY_PREFIX}{}",
            serde_json::to_string(&run.identity).map_err(encoding)?
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
    let first = boot_once(&paths)?;
    let second = boot_once(&paths)?;
    let fresh_entry_host_id = first.entry.host_id != second.entry.host_id;
    let fresh_entry_boot_id = first.entry.boot_id != second.entry.boot_id;
    let fresh_kernel_host_id = first.kernel.host_id != second.kernel.host_id;
    let fresh_kernel_boot_id = first.kernel.boot_id != second.kernel.boot_id;
    let stable_semantic_identities = first.kernel.source_document_id
        == second.kernel.source_document_id
        && first.kernel.checked_form_id == second.kernel.checked_form_id
        && first.kernel.expanded_form_id == second.kernel.expanded_form_id;
    let fresh_realization_identities = first.kernel.plan_id != second.kernel.plan_id
        && first.kernel.fragment_id != second.kernel.fragment_id
        && first.kernel.active_play_id != second.kernel.active_play_id;
    if !(fresh_entry_host_id && fresh_entry_boot_id && fresh_kernel_host_id && fresh_kernel_boot_id)
    {
        return Err(refusal(
            "stale-boot-identity",
            "independent IA-32 boots reused HostId or BootId",
        ));
    }
    if !stable_semantic_identities || !fresh_realization_identities {
        return Err(refusal(
            "wrong-identity-lifetime",
            "semantic identities changed or realization identities were reused across boots",
        ));
    }
    let (firmware, _) = firmware_paths(&paths)?;
    let snapshot_path = paths.target.join("a4-observatory-snapshot.json");
    fs::write(
        &snapshot_path,
        serde_json::to_vec_pretty(&first.observatory).map_err(encoding)?,
    )
    .map_err(|error| refusal("proof-record-failed", error.to_string()))?;
    prove_native_patchbay(&paths, &snapshot_path, &first.kernel)?;
    let proof = A3Proof {
        schema: "conduit.conduitos.ia32-a4-proof/v1",
        proof_class: "freestanding-ia32-emulator-observatory-patchbay",
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
        first: ProofRun {
            entry: first.entry,
            kernel: first.kernel,
            identity: first.identity,
        },
        second: ProofRun {
            entry: second.entry,
            kernel: second.kernel,
            identity: second.identity,
        },
        fresh_entry_host_id,
        fresh_entry_boot_id,
        fresh_kernel_host_id,
        fresh_kernel_boot_id,
        stable_semantic_identities,
        fresh_realization_identities,
        a3_ordinary_form_claimed: true,
        a4_observatory_patchbay_claimed: true,
        native_patchbay_consumed: true,
    };
    let proof_path = paths.target.join("a4-proof.json");
    fs::write(
        &proof_path,
        serde_json::to_vec_pretty(&proof).map_err(encoding)?,
    )
    .map_err(|error| refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&proof).map_err(encoding)?);
    } else if !opts.quiet {
        println!("ConduitOS IA-32 A4 proof: {}", proof_path.display());
    }
    Ok(())
}

fn boot_once(paths: &Paths) -> Result<A3Run, ConduitosError> {
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
            "-no-reboot",
            "-debugcon",
        ])
        .arg(format!("file:{}", transcript_path.display()))
        .args(["-global", "isa-debugcon.iobase=0xe9", "-drive"])
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
                    "emulator exited {status} before A3 Signs; stderr: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        let transcript = fs::read_to_string(&transcript_path).unwrap_or_default();
        if transcript.contains(ENTRY_PREFIX)
            && transcript.contains(KERNEL_PREFIX)
            && transcript.contains(IDENTITY_PREFIX)
            && transcript.contains(OBSERVATORY_PREFIX)
            && transcript.ends_with('\n')
        {
            child
                .kill()
                .map_err(|error| refusal("ia32-boot-failed", error.to_string()))?;
            child
                .wait()
                .map_err(|error| refusal("ia32-boot-failed", error.to_string()))?;
            let run = A3Run {
                entry: parse_one(&transcript, ENTRY_PREFIX, "entry")?,
                kernel: parse_one(&transcript, KERNEL_PREFIX, "kernel")?,
                identity: parse_one(&transcript, IDENTITY_PREFIX, "identity")?,
                observatory: parse_one(&transcript, OBSERVATORY_PREFIX, "Observatory")?,
            };
            validate(&run, paths)?;
            return Ok(run);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(refusal(
                "absent-ia32-a3-sign",
                format!("emulator timed out; transcript: {}", transcript.trim()),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn parse_one<T: for<'a> Deserialize<'a>>(
    transcript: &str,
    prefix: &str,
    name: &str,
) -> Result<T, ConduitosError> {
    let values: Vec<_> = transcript
        .split(prefix)
        .skip(1)
        .filter_map(|suffix| suffix.lines().next())
        .collect();
    if values.len() != 1 {
        return Err(refusal(
            "absent-or-duplicate-ia32-sign",
            format!("expected one {name} Sign, found {}", values.len()),
        ));
    }
    serde_json::from_str(values[0])
        .map_err(|error| refusal("malformed-ia32-sign", error.to_string()))
}

fn validate(run: &A3Run, paths: &Paths) -> Result<(), ConduitosError> {
    let commit = git_head(&paths.root)?;
    let entry = &run.entry;
    let kernel = &run.kernel;
    let identity = &run.identity;
    if entry.schema != "conduit.conduitos.ia32-entry-sign/v1"
        || entry.status != "entered"
        || entry.architecture != "ia32"
        || entry.build_id != commit
        || entry.image_id != format!("conduitos-image/{commit}/ia32/v1")
        || entry.bootloader != format!("Limine {LIMINE_VERSION}/BOOTIA32.EFI")
        || entry.emulator_profile != QEMU_PROFILE
        || !entry.host_id.starts_with("host-ia32-")
        || !entry.boot_id.starts_with("boot-ia32-")
        || kernel.schema != "conduit.conduitos.kernel-sign/v2"
        || kernel.status != "accepted"
        || kernel.arch != "ia32"
        || kernel.build_id != commit
        || kernel.pipeline != "check-plan-lower-kernel"
        || [
            &kernel.source_document_id,
            &kernel.checked_form_id,
            &kernel.expanded_form_id,
            &kernel.plan_id,
            &kernel.fragment_id,
            &kernel.active_play_id,
        ]
        .iter()
        .any(|id| id.is_empty())
        || kernel.semantic_result != "HELLO, CONDUITOS"
        || !kernel.allocation_stable_during_play
        || kernel.allocation_before_play != kernel.allocation_after_play
        || kernel.base_count != 7
        || kernel.execution_regions != 2
        || kernel.execution_lanes != 2
        || kernel.timer_slots != 1
        || kernel.interrupt_fact_slots == 0
        || kernel.timer_irq_wakes != 1
        || kernel.idle_entries == 0
        || kernel.serial_presentations != 2
        || kernel.pending_host_operations != 0
        || !kernel.overlap_witness
        || !kernel.timer_pending_during_text_progress
        || kernel.physical_parallelism
        || kernel.preemption
        || kernel.isolation
        || !kernel.sse2
        || kernel.rdrand
        || kernel.invariant_tsc
        || identity.image_id != format!("conduitos-image/{commit}/ia32/v1")
        || identity.wake_source != "8254-pit-channel0-irq0"
        || identity.wake_irq != 32
        || !identity.a3_ordinary_form_claimed
        || !identity.a4_observatory_patchbay_claimed
    {
        return Err(refusal("stale-or-invalid-ia32-a3-sign", "A3 Signs do not prove the exact portable Form, sealed Plan, finite Bases, real PIT wake, semantic result, and terminal Play"));
    }
    let observatory = &run.observatory;
    conduit_observatory::validate_snapshot(observatory)
        .map_err(|error| refusal("invalid-ia32-observatory", error))?;
    if observatory.schema != conduit_observatory::SNAPSHOT_SCHEMA
        || observatory.hosts.len() != 1
        || observatory.bases.len() != kernel.base_count
        || observatory.plans.len() != 1
        || observatory.plays.len() != 1
        || observatory.plans[0].plan_id.as_str() != kernel.plan_id
        || observatory.plans[0].source_document_id.as_str() != kernel.source_document_id
        || observatory.plans[0].checked_form_id.as_str() != kernel.checked_form_id
        || observatory.plans[0].expanded_form_id.as_str() != kernel.expanded_form_id
        || observatory.plays[0].active_play_id.as_str() != kernel.active_play_id
        || observatory.plays[0].boot_id.as_str() != kernel.boot_id
        || observatory.sealed_boot_provenance.len() != 1
        || observatory.sealed_boot_provenance[0].image_id.as_str() != identity.image_id
        || observatory.sealed_boot_provenance[0].firmware_environment != "uefi32"
    {
        return Err(refusal(
            "wrong-ia32-observatory-correlation",
            "ordinary snapshot does not correlate the exact IA-32 Form, Plan, Play, Bases, and boot provenance",
        ));
    }
    Ok(())
}

fn prove_native_patchbay(
    paths: &Paths,
    snapshot: &std::path::Path,
    kernel: &GuestKernelSign,
) -> Result<(), ConduitosError> {
    let snapshot = snapshot
        .to_str()
        .ok_or_else(|| refusal("patchbay-rejected-report", "non-UTF-8 path"))?;
    let output = super::profile::command(
        "cargo",
        &[
            "run",
            "--quiet",
            "-p",
            "patchbay-native",
            "--",
            "--linear-observatory-snapshot",
            snapshot,
        ],
        &paths.root,
        "patchbay-rejected-report",
    )?;
    let linear = String::from_utf8(output.stdout)
        .map_err(|error| refusal("patchbay-rejected-report", error.to_string()))?;
    for required in [
        kernel.host_id.as_str(),
        kernel.boot_id.as_str(),
        kernel.plan_id.as_str(),
        kernel.active_play_id.as_str(),
        "BASES 7",
        "SIGNS 19",
        "ExecutionRegionOverlap",
        "lifecycle=Completed",
        "proof=FreestandingEmulator",
    ] {
        if !linear.contains(required) {
            return Err(refusal(
                "patchbay-linear-projection-incomplete",
                format!("native Patchbay omitted {required}"),
            ));
        }
    }
    Ok(())
}

fn reject_dry_run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        Err(refusal(
            "dry-run-has-no-entry-sign",
            "IA-32 A3 requires UEFI emulator execution",
        ))
    } else {
        Ok(())
    }
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
    fn parser_rejects_absent_and_duplicate_signs() {
        assert!(parse_one::<EntrySign>("", ENTRY_PREFIX, "entry").is_err());
        let encoded = r#"{"schema":"x"}"#;
        assert!(parse_one::<EntrySign>(
            &format!("{ENTRY_PREFIX}{encoded}\n{ENTRY_PREFIX}{encoded}\n"),
            ENTRY_PREFIX,
            "entry"
        )
        .is_err());
    }
}
