use std::{fs, process::Command};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file},
    riscv64_a0, riscv64_a1, riscv64_a3, ConduitosArch, ConduitosError,
};

const BINARY: &str = "conduitos-riscv64-a4";
const OBSERVATORY_PREFIX: &str = "CONDUIT_OBSERVATORY_SNAPSHOT ";
const PROFILE: &str = "qemu-riscv64-virt-single-hart-256m-tcg-opensbi-uboot";

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
    first_kernel: riscv64_a3::KernelSign,
    second_kernel: riscv64_a3::KernelSign,
    first_identity: riscv64_a3::IdentitySign,
    second_identity: riscv64_a3::IdentitySign,
    stable_semantic_identities: bool,
    fresh_realization_identities: bool,
    fresh_host_id: bool,
    fresh_boot_id: bool,
    snapshot_schema: String,
    snapshot_bytes: usize,
    native_patchbay_consumed: bool,
    native_patchbay_linear_lines: usize,
}

pub fn run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    riscv64_a3::build_variant(opts, BINARY, "riscv64-a4")?;
    image::assemble(ConduitosArch::Riscv64, opts)?;
    let (kernel, identity, _) = boot_once(&paths)?;
    if opts.json {
        println!("{}", serde_json::to_string(&kernel).map_err(encoding)?);
    } else if !opts.quiet {
        println!("{}", serde_json::to_string(&kernel).map_err(encoding)?);
        println!("{}", serde_json::to_string(&identity).map_err(encoding)?);
    }
    Ok(())
}

pub fn prove(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    reject_dry_run(opts)?;
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    riscv64_a3::build_variant(opts, BINARY, "riscv64-a4")?;
    let image1 = image::assemble(ConduitosArch::Riscv64, opts)?;
    riscv64_a3::build_variant(opts, BINARY, "riscv64-a4")?;
    let image2 = image::assemble(ConduitosArch::Riscv64, opts)?;
    if image1.iso_sha256 != image2.iso_sha256 {
        return Err(refusal(
            "non-reproducible-image",
            "identical A4 inputs produced different images",
        ));
    }
    let (first_kernel, first_identity, snapshot) = boot_once(&paths)?;
    let (second_kernel, second_identity, _) = boot_once(&paths)?;
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
            "stale-or-collapsed-observatory-identities",
            "independent A4 boots collapsed semantic or realization identity stages",
        ));
    }
    let snapshot_path = paths.target.join("a4-observatory-snapshot.json");
    let snapshot_bytes = serde_json::to_vec_pretty(&snapshot).map_err(encoding)?;
    fs::write(&snapshot_path, &snapshot_bytes)
        .map_err(|e| refusal("proof-record-failed", e.to_string()))?;
    let native_patchbay_linear_lines =
        prove_native_patchbay(&paths, &snapshot_path, &first_kernel)?;
    let (qemu, _, _) = riscv64_a1::tools(&paths)?;
    let version = Command::new(qemu)
        .arg("--version")
        .output()
        .map_err(|e| refusal("unavailable-riscv64-emulator", e.to_string()))?;
    let proof = Proof {
        schema: "conduit.conduitos.riscv64-a4-proof/v1",
        proof_class: "freestanding-riscv64-ordinary-observatory-native-patchbay",
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
        first_kernel,
        second_kernel,
        first_identity,
        second_identity,
        stable_semantic_identities,
        fresh_realization_identities,
        fresh_host_id,
        fresh_boot_id,
        snapshot_schema: snapshot.schema.clone(),
        snapshot_bytes: snapshot_bytes.len(),
        native_patchbay_consumed: true,
        native_patchbay_linear_lines,
    };
    let path = paths.target.join("a4-proof.json");
    fs::write(&path, serde_json::to_vec_pretty(&proof).map_err(encoding)?)
        .map_err(|e| refusal("proof-record-failed", e.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&proof).map_err(encoding)?);
    } else if !opts.quiet {
        println!("ConduitOS RISC-V64 A4 proof: {}", path.display());
    }
    Ok(())
}

fn boot_once(
    paths: &Paths,
) -> Result<
    (
        riscv64_a3::KernelSign,
        riscv64_a3::IdentitySign,
        conduit_observatory::ObservatorySnapshot,
    ),
    ConduitosError,
> {
    let text = riscv64_a3::boot_transcript(paths)?;
    let entry = riscv64_a1::parse(&text)?;
    riscv64_a1::validate(&entry, paths)?;
    let kernel = riscv64_a3::parse_one(&text, "CONDUIT_KERNEL_SIGN ", "kernel")?;
    let identity = riscv64_a3::parse_one(&text, "CONDUIT_RISCV64_A3_IDENTITY ", "identity")?;
    riscv64_a3::validate(&kernel, &identity, paths, true)?;
    let snapshot: conduit_observatory::ObservatorySnapshot =
        riscv64_a3::parse_one(&text, OBSERVATORY_PREFIX, "Observatory")?;
    conduit_observatory::validate_snapshot(&snapshot)
        .map_err(|e| refusal("invalid-riscv64-observatory-snapshot", e.to_string()))?;
    validate_correlation(&kernel, &snapshot)?;
    Ok((kernel, identity, snapshot))
}

fn validate_correlation(
    kernel: &riscv64_a3::KernelSign,
    snapshot: &conduit_observatory::ObservatorySnapshot,
) -> Result<(), ConduitosError> {
    let host = snapshot
        .hosts
        .first()
        .ok_or_else(|| refusal("broken-riscv64-observatory-correlation", "host absent"))?;
    let plan = snapshot
        .plans
        .first()
        .ok_or_else(|| refusal("broken-riscv64-observatory-correlation", "plan absent"))?;
    let play = snapshot
        .plays
        .first()
        .ok_or_else(|| refusal("broken-riscv64-observatory-correlation", "play absent"))?;
    if snapshot.hosts.len() != 1
        || snapshot.plans.len() != 1
        || snapshot.plays.len() != 1
        || host.advertisement.host_id.as_str() != kernel.host_id
        || host.advertisement.boot_id.as_str() != kernel.boot_id
        || plan.plan_id.as_str() != kernel.plan_id
        || play.plan_id.as_str() != kernel.plan_id
        || play.active_play_id.as_str() != kernel.active_play_id
        || snapshot.bases.len() != 7
        || snapshot.retention.dropped_items != 0
    {
        return Err(refusal(
            "broken-riscv64-observatory-correlation",
            "snapshot does not exactly correlate Host/Boot/Base/Plan/Play truth",
        ));
    }
    Ok(())
}

fn prove_native_patchbay(
    paths: &Paths,
    snapshot: &std::path::Path,
    kernel: &riscv64_a3::KernelSign,
) -> Result<usize, ConduitosError> {
    let output = super::profile::command(
        "cargo",
        &[
            "run",
            "--quiet",
            "-p",
            "patchbay-native",
            "--",
            "--linear-observatory-snapshot",
            snapshot.to_str().unwrap(),
        ],
        &paths.root,
        "patchbay-rejected-riscv64-snapshot",
    )?;
    let linear = String::from_utf8(output.stdout)
        .map_err(|e| refusal("patchbay-rejected-riscv64-snapshot", e.to_string()))?;
    for required in [
        kernel.host_id.as_str(),
        kernel.boot_id.as_str(),
        kernel.plan_id.as_str(),
        kernel.fragment_id.as_str(),
        kernel.active_play_id.as_str(),
        "BASES 7",
        "SIGNS 19",
        "lifecycle=Completed",
        "visible_gaps=0",
        "firmware=sbi",
        "proof=FreestandingEmulator",
    ] {
        if !linear.contains(required) {
            return Err(refusal(
                "patchbay-riscv64-projection-incomplete",
                format!("native Patchbay omitted {required}"),
            ));
        }
    }
    Ok(linear.lines().count())
}

fn reject_dry_run(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        Err(refusal(
            "dry-run-has-no-a4-proof",
            "RISC-V64 A4 requires emulator and native Patchbay execution",
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
