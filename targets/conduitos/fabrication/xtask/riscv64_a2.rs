use std::{fs, process::Command};

use serde::{Deserialize, Serialize};

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file, ArtifactRole, BuildRecord},
    riscv64_a0, riscv64_a1, ConduitosArch, ConduitosError,
};

const BINARY: &str = "conduitos-riscv64-a2";
const PREFIX: &str = "CONDUIT_RISCV64_MACHINE_SIGN ";
const PROFILE: &str = "qemu-riscv64-virt-single-hart-256m-tcg-opensbi-uboot";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MachineSign {
    schema: String,
    status: String,
    architecture: String,
    boot_id: String,
    kernel: String,
    lane_id: String,
    lane_count: u32,
    runtime_base_count: u32,
    runtime_memory_bytes: u32,
    timer_slots: u32,
    interrupt_fact_slots: u32,
    wake_source: String,
    wake_cause: u32,
    sbi_mechanism: String,
    idle_entries: u32,
    timer_wakes: u32,
    kernel_decisions: u32,
    kernel_signs: u32,
    pending_host_operations: u32,
    sequence: Vec<String>,
    a3_ordinary_form_claimed: bool,
}

#[derive(Serialize)]
struct Proof {
    schema: &'static str,
    proof_class: &'static str,
    base_commit: String,
    architecture: &'static str,
    rust_target: &'static str,
    artifact_sha256: String,
    image_sha256: String,
    reproducible_image: bool,
    limine_version: &'static str,
    limine_archive_sha256: &'static str,
    emulator_profile: &'static str,
    qemu_version: String,
    opensbi: String,
    uboot: String,
    first: MachineSign,
    second: MachineSign,
    fresh_boot_id: bool,
    production_kernel_owner: bool,
    real_timer_wake: bool,
    a3_ordinary_form_claimed: bool,
}

pub fn run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    build(opts)?;
    image::assemble_architecture_proof(ConduitosArch::Riscv64, opts)?;
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
    build(opts)?;
    let image1 = image::assemble_architecture_proof(ConduitosArch::Riscv64, opts)?;
    build(opts)?;
    let image2 = image::assemble_architecture_proof(ConduitosArch::Riscv64, opts)?;
    if image1.iso_sha256 != image2.iso_sha256 {
        return Err(refusal(
            "non-reproducible-image",
            "identical A2 inputs produced different images",
        ));
    }
    let first = boot_once(&paths)?;
    let second = boot_once(&paths)?;
    let fresh_boot_id = first.boot_id != second.boot_id;
    if !fresh_boot_id {
        return Err(refusal(
            "stale-boot-identity",
            "independent A2 boots reused BootId",
        ));
    }
    let (qemu, opensbi, uboot) = riscv64_a1::tools(&paths)?;
    let version = Command::new(&qemu)
        .arg("--version")
        .output()
        .map_err(|e| refusal("unavailable-riscv64-emulator", e.to_string()))?;
    let proof = Proof {
        schema: "conduit.conduitos.riscv64-a2-proof/v1",
        proof_class: "freestanding-riscv64-production-kernel-real-timer-wake",
        base_commit: git_head(&paths.root)?,
        architecture: "riscv64",
        rust_target: riscv64_a0::TARGET,
        artifact_sha256: sha256_file(&paths.kernel)?,
        image_sha256: image1.iso_sha256,
        reproducible_image: true,
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        emulator_profile: PROFILE,
        qemu_version: String::from_utf8_lossy(&version.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .into(),
        opensbi: opensbi.display().to_string(),
        uboot: uboot.display().to_string(),
        first,
        second,
        fresh_boot_id,
        production_kernel_owner: true,
        real_timer_wake: true,
        a3_ordinary_form_claimed: false,
    };
    let path = paths.target.join("a2-proof.json");
    fs::write(&path, serde_json::to_vec_pretty(&proof).map_err(encoding)?)
        .map_err(|e| refusal("proof-record-failed", e.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&proof).map_err(encoding)?);
    } else if !opts.quiet {
        println!("ConduitOS RISC-V64 A2 proof: {}", path.display());
    }
    Ok(())
}

fn build(opts: &GlobalOpts) -> Result<BuildRecord, ConduitosError> {
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    fs::create_dir_all(&paths.target)
        .map_err(|e| refusal("build-output-unavailable", e.to_string()))?;
    let commit = git_head(&paths.root)?;
    let linker = riscv64_a0::rust_lld(&paths.root)?;
    let script = paths
        .root
        .join("targets/conduitos/proof/appliances/riscv64/linker/a2.ld");
    let rustflags = format!("-C relocation-model=static -C panic=abort -C linker={} -C link-arg=-T{} -C link-arg=--nostdlib", linker.display(), script.display());
    let mut command = Command::new("cargo");
    command
        .args([
            "build",
            "-p",
            "conduitos",
            "--bin",
            BINARY,
            "--features",
            "riscv64-a2",
            "--target",
            riscv64_a0::TARGET,
            "--release",
        ])
        .current_dir(&paths.root)
        .env("RUSTFLAGS", rustflags)
        .env("CONDUITOS_BUILD_ID", &commit)
        .env(
            "CONDUITOS_IMAGE_ID",
            format!("conduitos-image/{commit}/riscv64/v1"),
        );
    if opts.locked {
        command.arg("--locked");
    }
    let status = command
        .status()
        .map_err(|e| refusal("riscv64-a2-toolchain-unavailable", e.to_string()))?;
    if !status.success() {
        return Err(refusal(
            "riscv64-a2-compile-link-failed",
            status.to_string(),
        ));
    }
    let built = paths
        .root
        .join(format!("target/{}/release/{BINARY}", riscv64_a0::TARGET));
    fs::copy(built, &paths.kernel)
        .map_err(|e| refusal("build-output-unavailable", e.to_string()))?;
    let symbols = super::profile::command(
        "readelf",
        &["-sW", paths.kernel.to_str().unwrap()],
        &paths.root,
        "readelf-unavailable",
    )?;
    if !String::from_utf8_lossy(&symbols.stdout)
        .lines()
        .any(|line| line.contains("GLOBAL") && line.ends_with("conduitos_riscv64_a2_start"))
    {
        return Err(refusal(
            "invalid-riscv64-a2-artifact",
            "exact A2 entry is absent",
        ));
    }
    let record = BuildRecord {
        schema: "conduit.conduitos.build/v2",
        artifact_role: ArtifactRole::ArchitectureProofAppliance,
        base_commit: commit,
        architecture: "riscv64",
        rust_target: riscv64_a0::TARGET,
        limine_crate: "0.5.0",
        elf_sha256: sha256_file(&paths.kernel)?,
    };
    fs::write(
        paths.target.join("build.json"),
        serde_json::to_vec_pretty(&record).map_err(encoding)?,
    )
    .map_err(|e| refusal("build-record-failed", e.to_string()))?;
    Ok(record)
}

fn boot_once(paths: &Paths) -> Result<MachineSign, ConduitosError> {
    let text = riscv64_a1::boot_until(paths, PREFIX)?;
    let entry = riscv64_a1::parse(&text)?;
    riscv64_a1::validate(&entry, paths)?;
    let sign = parse(&text)?;
    validate(&sign)?;
    Ok(sign)
}

fn parse(text: &str) -> Result<MachineSign, ConduitosError> {
    let values: Vec<_> = text
        .split(PREFIX)
        .skip(1)
        .filter_map(|s| s.lines().next())
        .collect();
    if values.len() != 1 {
        return Err(refusal(
            "absent-or-duplicate-riscv64-machine-sign",
            format!("expected one machine Sign, found {}", values.len()),
        ));
    }
    serde_json::from_str(values[0].trim_end_matches('\r'))
        .map_err(|e| refusal("malformed-riscv64-machine-sign", e.to_string()))
}

fn validate(sign: &MachineSign) -> Result<(), ConduitosError> {
    let sequence = [
        "machine-init",
        "lane-handoff",
        "idle",
        "timer-wake",
        "terminal",
    ];
    if sign.schema != "conduit.conduitos.riscv64-a2-sign/v1"
        || sign.status != "completed"
        || sign.architecture != "riscv64"
        || !sign.boot_id.starts_with("boot-riscv64-")
        || sign.kernel != "conduit-kernel"
        || sign.lane_id != "lane/riscv64/cooperative/0"
        || sign.lane_count != 1
        || sign.runtime_base_count != 4
        || sign.runtime_memory_bytes != 4096
        || sign.timer_slots != 1
        || sign.interrupt_fact_slots != 1
        || sign.wake_source != "riscv-supervisor-timer-interrupt"
        || sign.wake_cause != 5
        || sign.sbi_mechanism != "TIME/set_timer"
        || sign.idle_entries == 0
        || sign.timer_wakes != 1
        || sign.kernel_decisions == 0
        || sign.kernel_signs == 0
        || sign.pending_host_operations != 0
        || sign.sequence.iter().map(String::as_str).ne(sequence)
        || sign.a3_ordinary_form_claimed
    {
        return Err(refusal(
            "stale-or-invalid-riscv64-machine-sign",
            "machine Sign violates the exact A2 contract",
        ));
    }
    Ok(())
}

fn reject_dry_run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        Err(refusal(
            "dry-run-has-no-machine-sign",
            "RISC-V64 A2 requires emulator execution",
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
    fn absent_and_duplicate_machine_signs_refuse() {
        assert!(parse("").is_err());
        assert!(parse(&format!("{PREFIX}{{}}\n{PREFIX}{{}}\n")).is_err());
    }
}
