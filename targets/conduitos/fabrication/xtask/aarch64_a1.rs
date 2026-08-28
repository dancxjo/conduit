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
const SIGN_PREFIX: &str = "CONDUIT_KERNEL_SIGN ";
const IDENTITY_PREFIX: &str = "CONDUIT_AARCH64_A3_IDENTITY ";
const OBSERVATORY_PREFIX: &str = "CONDUIT_OBSERVATORY_SNAPSHOT ";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct KernelSign {
    schema: String,
    status: String,
    arch: String,
    build_id: String,
    host_id: String,
    boot_id: String,
    pipeline: String,
    source_document_id: String,
    checked_form_id: String,
    expanded_form_id: String,
    plan_id: String,
    fragment_id: String,
    active_play_id: String,
    semantic_result: String,
    allocation_before_play: usize,
    allocation_after_play: usize,
    allocation_stable_during_play: bool,
    base_count: u32,
    execution_regions: u32,
    execution_lanes: u32,
    timer_slots: u32,
    interrupt_fact_slots: u32,
    timer_irq_wakes: u32,
    idle_entries: u32,
    serial_presentations: u32,
    pending_host_operations: u32,
    overlap_witness: bool,
    timer_pending_during_text_progress: bool,
    physical_parallelism: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct IdentitySign {
    image_id: String,
    wake_source: String,
    wake_irq: u32,
    a3_ordinary_form_claimed: bool,
}

#[derive(Serialize)]
struct A3Proof {
    schema: &'static str,
    proof_class: &'static str,
    base_commit: String,
    architecture: &'static str,
    rust_target: &'static str,
    limine_version: &'static str,
    limine_archive_sha256: &'static str,
    emulator_profile: &'static str,
    firmware: String,
    qemu_version: String,
    iso_sha256: String,
    reproducible_image: bool,
    first_kernel: KernelSign,
    second_kernel: KernelSign,
    first_identity: IdentitySign,
    second_identity: IdentitySign,
    fresh_host_id: bool,
    fresh_boot_id: bool,
    a3_ordinary_form_claimed: bool,
    a4_observatory_patchbay_claimed: bool,
    native_patchbay_consumed: bool,
}

pub fn run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let _ = image::execute_architecture_proof(ConduitosArch::Aarch64, opts)?;
    let (kernel, identity, _) = boot_once(&paths)?;
    if opts.json {
        println!("{}", serde_json::to_string(&kernel).map_err(encoding)?);
    } else if !opts.quiet {
        println!(
            "{SIGN_PREFIX}{}",
            serde_json::to_string(&kernel).map_err(encoding)?
        );
        println!(
            "{IDENTITY_PREFIX}{}",
            serde_json::to_string(&identity).map_err(encoding)?
        );
    }
    Ok(())
}

pub fn prove(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-entry-sign",
            "AArch64 A3 proof requires emulator execution",
        ));
    }
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let first_image = image::execute_architecture_proof(ConduitosArch::Aarch64, opts)?;
    let second_image = image::execute_architecture_proof(ConduitosArch::Aarch64, opts)?;
    if first_image.iso_sha256 != second_image.iso_sha256 {
        return Err(ConduitosError::refusal(
            "non-reproducible-image",
            "identical AArch64 inputs produced different images",
        ));
    }
    let (first_kernel, first_identity, first_observatory) = boot_once(&paths)?;
    let (second_kernel, second_identity, _) = boot_once(&paths)?;
    let fresh_host_id = first_kernel.host_id != second_kernel.host_id;
    let fresh_boot_id = first_kernel.boot_id != second_kernel.boot_id;
    if !fresh_host_id || !fresh_boot_id {
        return Err(ConduitosError::refusal(
            "stale-boot-identity",
            "independent AArch64 boots reused HostId or BootId",
        ));
    }
    let snapshot_path = paths.target.join("a4-observatory-snapshot.json");
    fs::write(
        &snapshot_path,
        serde_json::to_vec_pretty(&first_observatory).map_err(encoding)?,
    )
    .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    prove_native_patchbay(&paths, &snapshot_path, &first_kernel)?;
    let proof = A3Proof {
        schema: "conduit.conduitos.aarch64-a4-proof/v1",
        proof_class: "freestanding-emulator-observatory-patchbay",
        base_commit: git_head(&paths.root)?,
        architecture: "aarch64",
        rust_target: super::aarch64_a0::TARGET,
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        emulator_profile: AARCH64_QEMU_PROFILE,
        firmware: firmware_path().to_string_lossy().into_owned(),
        qemu_version: qemu_version(&paths)?,
        iso_sha256: sha256_file(&paths.iso)?,
        reproducible_image: true,
        first_kernel,
        second_kernel,
        first_identity,
        second_identity,
        fresh_host_id,
        fresh_boot_id,
        a3_ordinary_form_claimed: true,
        a4_observatory_patchbay_claimed: true,
        native_patchbay_consumed: true,
    };
    fs::write(
        paths.target.join("a4-proof.json"),
        serde_json::to_vec_pretty(&proof).map_err(encoding)?,
    )
    .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&proof).map_err(encoding)?);
    } else if !opts.quiet {
        println!(
            "ConduitOS AArch64 A4 proof: {}",
            paths.target.join("a4-proof.json").display()
        );
    }
    Ok(())
}

fn boot_once(
    paths: &Paths,
) -> Result<
    (
        KernelSign,
        IdentitySign,
        conduit_observatory::ObservatorySnapshot,
    ),
    ConduitosError,
> {
    let firmware = firmware_path();
    if !paths.limine.join("BOOTAA64.EFI").is_file() || !firmware.is_file() {
        return Err(ConduitosError::refusal(
            "unavailable-aarch64-firmware",
            "required Limine or UEFI artifact is absent",
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
        ])
        .arg(&firmware)
        .args(["-cdrom"])
        .arg(&paths.iso)
        .args(["-boot", "d"])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            ConduitosError::refusal("unavailable-aarch64-emulator", error.to_string())
        })?;
    let deadline = Instant::now() + Duration::from_secs(60);
    while child
        .try_wait()
        .map_err(|error| ConduitosError::refusal("aarch64-boot-failed", error.to_string()))?
        .is_none()
    {
        if Instant::now() >= deadline {
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child
        .wait_with_output()
        .map_err(|error| ConduitosError::refusal("aarch64-boot-failed", error.to_string()))?;
    let mut transcript = output.stdout;
    transcript.extend_from_slice(&output.stderr);
    let serial = String::from_utf8_lossy(&transcript);
    let kernel: KernelSign = parse_one(&serial, SIGN_PREFIX, "kernel")?;
    let identity: IdentitySign = parse_one(&serial, IDENTITY_PREFIX, "identity")?;
    let observatory = parse_one(&serial, OBSERVATORY_PREFIX, "Observatory")?;
    validate(&kernel, &identity, &observatory, paths)?;
    Ok((kernel, identity, observatory))
}

fn parse_one<T: for<'a> Deserialize<'a>>(
    serial: &str,
    prefix: &str,
    name: &str,
) -> Result<T, ConduitosError> {
    let values: Vec<_> = serial
        .split(prefix)
        .skip(1)
        .filter_map(|suffix| suffix.lines().next())
        .collect();
    if values.len() != 1 {
        return Err(ConduitosError::refusal(
            "absent-aarch64-sign",
            format!(
                "expected one {name} Sign, found {}; transcript: {}",
                values.len(),
                serial.trim()
            ),
        ));
    }
    serde_json::from_str(values[0])
        .map_err(|error| ConduitosError::refusal("malformed-aarch64-sign", error.to_string()))
}

fn validate(
    kernel: &KernelSign,
    identity: &IdentitySign,
    observatory: &conduit_observatory::ObservatorySnapshot,
    paths: &Paths,
) -> Result<(), ConduitosError> {
    let commit = git_head(&paths.root)?;
    if kernel.schema != "conduit.conduitos.kernel-sign/v2"
        || kernel.status != "accepted"
        || kernel.arch != "aarch64"
        || kernel.build_id != format!("conduitos-build/{commit}/aarch64/v1")
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
        || identity.image_id != format!("conduitos-image/{commit}/aarch64/v1")
        || identity.wake_source != "arm-generic-virtual-timer-ppi-27"
        || identity.wake_irq != 27
        || !identity.a3_ordinary_form_claimed
    {
        return Err(ConduitosError::refusal("stale-or-invalid-aarch64-a3-sign", "A3 Sign does not prove the exact portable Form, Plan, Bases, wake, semantic result, and terminal Play"));
    }
    conduit_observatory::validate_snapshot(observatory)
        .map_err(|error| ConduitosError::refusal("invalid-aarch64-observatory", error))?;
    if observatory.schema != conduit_observatory::SNAPSHOT_SCHEMA
        || observatory.hosts.len() != 1
        || observatory.bases.len() != kernel.base_count as usize
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
    {
        return Err(ConduitosError::refusal(
            "wrong-aarch64-observatory-correlation",
            "ordinary snapshot does not correlate the exact AArch64 Form, Plan, Play, Bases, and boot provenance",
        ));
    }
    Ok(())
}

fn prove_native_patchbay(
    paths: &Paths,
    snapshot: &std::path::Path,
    kernel: &KernelSign,
) -> Result<(), ConduitosError> {
    let snapshot = snapshot
        .to_str()
        .ok_or_else(|| ConduitosError::refusal("patchbay-rejected-report", "non-UTF-8 path"))?;
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
        .map_err(|error| ConduitosError::refusal("patchbay-rejected-report", error.to_string()))?;
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
            return Err(ConduitosError::refusal(
                "patchbay-linear-projection-incomplete",
                format!("native Patchbay omitted {required}"),
            ));
        }
    }
    Ok(())
}

fn firmware_path() -> std::path::PathBuf {
    std::env::var_os("CONDUITOS_AARCH64_UEFI_FIRMWARE")
        .map(Into::into)
        .unwrap_or_else(|| FIRMWARE.into())
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
