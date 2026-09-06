use std::{fs, process::Command};

use serde::{Deserialize, Serialize};

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file, ArtifactRole, BuildRecord},
    riscv64_a0, riscv64_a1, ConduitosArch, ConduitosError,
};

const BINARY: &str = "conduitos-riscv64-a3";
const KERNEL_PREFIX: &str = "CONDUIT_KERNEL_SIGN ";
const IDENTITY_PREFIX: &str = "CONDUIT_RISCV64_A3_IDENTITY ";
const PROFILE: &str = "qemu-riscv64-virt-single-hart-256m-tcg-opensbi-uboot";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct KernelSign {
    schema: String,
    status: String,
    arch: String,
    build_id: String,
    kernel: String,
    scheduler_profile: String,
    pub(super) host_id: String,
    pub(super) boot_id: String,
    pipeline: String,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
    pub(super) expanded_form_id: String,
    pub(super) plan_id: String,
    pub(super) fragment_id: String,
    pub(super) active_play_id: String,
    planned_sign_items: u32,
    planned_sign_bytes: u32,
    cord_item_capacity: u32,
    cord_byte_capacity: u32,
    semantic_result: String,
    allocation_before_play: usize,
    allocation_after_play: usize,
    allocation_capacity: usize,
    allocation_stable_during_play: bool,
    base_ids: Vec<String>,
    base_count: u32,
    memory_arena_bytes: u64,
    execution_regions: u32,
    execution_lanes: u32,
    region_ids: Vec<String>,
    lane_resource_ids: Vec<String>,
    lane_base_id: String,
    timer_slots: u32,
    serial_slots: u32,
    interrupt_fact_slots: u32,
    sign_item_slots: u32,
    logical_operations: u32,
    kernel_decisions: u32,
    kernel_signs: u32,
    timer_irq_wakes: u32,
    idle_entries: u32,
    serial_presentations: u32,
    clock_monotonic: bool,
    pending_host_operations: u32,
    overlap_witness: bool,
    timer_pending_during_text_progress: bool,
    physical_parallelism: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct IdentitySign {
    image_id: String,
    wake_source: String,
    wake_cause: u32,
    sbi_mechanism: String,
    a3_ordinary_form_claimed: bool,
    pub(super) a4_observatory_patchbay_claimed: bool,
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
    first_kernel: KernelSign,
    second_kernel: KernelSign,
    first_identity: IdentitySign,
    second_identity: IdentitySign,
    stable_semantic_identities: bool,
    fresh_realization_identities: bool,
    fresh_host_id: bool,
    fresh_boot_id: bool,
    a4_observatory_patchbay_claimed: bool,
}

pub fn run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    build_variant(opts, BINARY, "riscv64-a3")?;
    image::assemble_architecture_proof(ConduitosArch::Riscv64, opts)?;
    let (kernel, identity) = boot_once(&paths)?;
    if opts.json {
        println!("{}", serde_json::to_string(&kernel).map_err(encoding)?);
    } else if !opts.quiet {
        println!(
            "{KERNEL_PREFIX}{}",
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
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    build_variant(opts, BINARY, "riscv64-a3")?;
    let first_image = image::assemble_architecture_proof(ConduitosArch::Riscv64, opts)?;
    build_variant(opts, BINARY, "riscv64-a3")?;
    let second_image = image::assemble_architecture_proof(ConduitosArch::Riscv64, opts)?;
    if first_image.iso_sha256 != second_image.iso_sha256 {
        return Err(refusal(
            "non-reproducible-image",
            "identical A3 inputs produced different images",
        ));
    }
    let (first_kernel, first_identity) = boot_once(&paths)?;
    let (second_kernel, second_identity) = boot_once(&paths)?;
    let stable_semantic_identities = first_kernel.source_document_id
        == second_kernel.source_document_id
        && first_kernel.checked_form_id == second_kernel.checked_form_id
        && first_kernel.expanded_form_id == second_kernel.expanded_form_id;
    let fresh_realization_identities = first_kernel.plan_id != second_kernel.plan_id
        && first_kernel.fragment_id != second_kernel.fragment_id
        && first_kernel.active_play_id != second_kernel.active_play_id;
    let fresh_host_id = first_kernel.host_id != second_kernel.host_id;
    let fresh_boot_id = first_kernel.boot_id != second_kernel.boot_id;
    if !stable_semantic_identities
        || !fresh_realization_identities
        || !fresh_host_id
        || !fresh_boot_id
    {
        return Err(refusal(
            "stale-or-collapsed-play-identities",
            "two boots did not preserve semantic and refresh realization identities",
        ));
    }
    let (qemu, _, _) = riscv64_a1::tools(&paths)?;
    let version = Command::new(qemu)
        .arg("--version")
        .output()
        .map_err(|e| refusal("unavailable-riscv64-emulator", e.to_string()))?;
    let proof = Proof {
        schema: "conduit.conduitos.riscv64-a3-proof/v1",
        proof_class: "freestanding-riscv64-ordinary-form-plan-play",
        base_commit: git_head(&paths.root)?,
        architecture: "riscv64",
        rust_target: riscv64_a0::TARGET,
        artifact_sha256: sha256_file(&paths.kernel)?,
        image_sha256: first_image.iso_sha256,
        reproducible_image: true,
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        emulator_profile: PROFILE,
        qemu_version: String::from_utf8_lossy(&version.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .into(),
        first_kernel,
        second_kernel,
        first_identity,
        second_identity,
        stable_semantic_identities,
        fresh_realization_identities,
        fresh_host_id,
        fresh_boot_id,
        a4_observatory_patchbay_claimed: false,
    };
    let path = paths.target.join("a3-proof.json");
    fs::write(&path, serde_json::to_vec_pretty(&proof).map_err(encoding)?)
        .map_err(|e| refusal("proof-record-failed", e.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&proof).map_err(encoding)?);
    } else if !opts.quiet {
        println!("ConduitOS RISC-V64 A3 proof: {}", path.display());
    }
    Ok(())
}

pub(super) fn build_variant(
    opts: &GlobalOpts,
    binary: &str,
    feature: &str,
) -> Result<BuildRecord, ConduitosError> {
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    fs::create_dir_all(&paths.target)
        .map_err(|e| refusal("build-output-unavailable", e.to_string()))?;
    let commit = git_head(&paths.root)?;
    let linker = riscv64_a0::rust_lld(&paths.root)?;
    let script = paths
        .root
        .join("targets/conduitos/proof/appliances/riscv64/linker/a3.ld");
    let rustflags = format!("-C relocation-model=static -C panic=abort -C linker={} -C link-arg=-T{} -C link-arg=--nostdlib", linker.display(), script.display());
    let mut command = Command::new("cargo");
    command
        .args([
            "build",
            "-p",
            "conduitos",
            "--bin",
            binary,
            "--features",
            feature,
            "--target",
            riscv64_a0::TARGET,
            "--release",
        ])
        .current_dir(&paths.root)
        .env("RUSTFLAGS", rustflags)
        .env(
            "CONDUITOS_BUILD_ID",
            format!("conduitos-build/{commit}/riscv64/v1"),
        )
        .env("CONDUITOS_ARTIFACT_COMMIT", &commit)
        .env(
            "CONDUITOS_IMAGE_ID",
            format!("conduitos-image/{commit}/riscv64/v1"),
        );
    if opts.locked {
        command.arg("--locked");
    }
    let status = command
        .status()
        .map_err(|e| refusal("riscv64-a3-toolchain-unavailable", e.to_string()))?;
    if !status.success() {
        return Err(refusal(
            "riscv64-a3-compile-link-failed",
            status.to_string(),
        ));
    }
    let built = paths
        .root
        .join(format!("target/{}/release/{binary}", riscv64_a0::TARGET));
    fs::copy(built, &paths.kernel)
        .map_err(|e| refusal("build-output-unavailable", e.to_string()))?;
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

fn boot_once(paths: &Paths) -> Result<(KernelSign, IdentitySign), ConduitosError> {
    let text = boot_transcript(paths)?;
    let entry = riscv64_a1::parse(&text)?;
    riscv64_a1::validate(&entry, paths)?;
    let kernel = parse_one(&text, KERNEL_PREFIX, "kernel")?;
    let identity = parse_one(&text, IDENTITY_PREFIX, "identity")?;
    validate(&kernel, &identity, paths, false)?;
    Ok((kernel, identity))
}

pub(super) fn boot_transcript(paths: &Paths) -> Result<String, ConduitosError> {
    riscv64_a1::boot_until(paths, IDENTITY_PREFIX)
}

pub(super) fn parse_one<T: for<'a> Deserialize<'a>>(
    text: &str,
    prefix: &str,
    name: &str,
) -> Result<T, ConduitosError> {
    let values: Vec<_> = text
        .split(prefix)
        .skip(1)
        .filter_map(|s| s.lines().next())
        .collect();
    if values.len() != 1 {
        return Err(refusal(
            "absent-or-duplicate-riscv64-a3-sign",
            format!("expected one {name} Sign, found {}", values.len()),
        ));
    }
    serde_json::from_str(values[0].trim_end_matches('\r'))
        .map_err(|e| refusal("malformed-riscv64-a3-sign", e.to_string()))
}

pub(super) fn validate(
    kernel: &KernelSign,
    identity: &IdentitySign,
    paths: &Paths,
    expected_a4: bool,
) -> Result<(), ConduitosError> {
    let commit = git_head(&paths.root)?;
    let exact_id =
        |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    let exact_base_ids = kernel.base_ids.len() == 7
        && kernel
            .base_ids
            .iter()
            .enumerate()
            .all(|(index, id)| exact_id(id) && !kernel.base_ids[..index].contains(id));
    if kernel.schema != "conduit.conduitos.kernel-sign/v2"
        || kernel.status != "accepted"
        || kernel.arch != "riscv64"
        || kernel.build_id != format!("conduitos-build/{commit}/riscv64/v1")
        || kernel.kernel != "conduit-kernel"
        || kernel.scheduler_profile != "conduitos/two-lane-cooperative@1"
        || kernel.pipeline != "check-plan-lower-kernel"
        || ![
            &kernel.source_document_id,
            &kernel.checked_form_id,
            &kernel.expanded_form_id,
            &kernel.plan_id,
            &kernel.fragment_id,
            &kernel.active_play_id,
        ]
        .into_iter()
        .all(|id| exact_id(id))
        || kernel.planned_sign_items == 0
        || kernel.planned_sign_bytes == 0
        || kernel.cord_item_capacity != 3
        || kernel.cord_byte_capacity != 192
        || kernel.semantic_result != "HELLO, CONDUITOS"
        || !kernel.allocation_stable_during_play
        || kernel.allocation_before_play != kernel.allocation_after_play
        || kernel.allocation_capacity != 1024 * 1024
        || !exact_base_ids
        || kernel.base_count != 7
        || kernel.memory_arena_bytes != 1024 * 1024
        || kernel.execution_regions != 2
        || kernel.execution_lanes != 2
        || kernel.region_ids != ["region/text", "region/timer"]
        || kernel.lane_resource_ids.len() != 2
        || kernel.lane_resource_ids[0] == kernel.lane_resource_ids[1]
        || kernel.lane_resource_ids.iter().any(String::is_empty)
        || !exact_id(&kernel.lane_base_id)
        || kernel.timer_slots != 1
        || kernel.serial_slots != 2
        || kernel.interrupt_fact_slots != 4
        || kernel.sign_item_slots != 64
        || kernel.logical_operations != 5
        || kernel.kernel_decisions == 0
        || kernel.kernel_signs == 0
        || kernel.timer_irq_wakes != 1
        || kernel.idle_entries == 0
        || kernel.serial_presentations != 2
        || !kernel.clock_monotonic
        || kernel.pending_host_operations != 0
        || !kernel.overlap_witness
        || !kernel.timer_pending_during_text_progress
        || kernel.physical_parallelism
        || identity.image_id != format!("conduitos-image/{commit}/riscv64/v1")
        || identity.wake_source != "riscv-supervisor-timer-interrupt"
        || identity.wake_cause != 5
        || identity.sbi_mechanism != "TIME/set_timer"
        || !identity.a3_ordinary_form_claimed
        || identity.a4_observatory_patchbay_claimed != expected_a4
    {
        return Err(refusal(
            "stale-or-invalid-riscv64-a3-sign",
            "ordinary Form/Plan/Play proof violates the exact A3 contract",
        ));
    }
    Ok(())
}

fn reject_dry_run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        Err(refusal(
            "dry-run-has-no-a3-sign",
            "RISC-V64 A3 requires emulator execution",
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
    fn absent_a3_sign_refuses() {
        assert!(parse_one::<KernelSign>("", KERNEL_PREFIX, "kernel").is_err());
    }
}
