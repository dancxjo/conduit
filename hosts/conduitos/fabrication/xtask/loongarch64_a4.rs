use std::{fs, process::Command};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    image, loongarch64_a0, loongarch64_a1, loongarch64_a2, loongarch64_a3,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file},
    ConduitosArch, ConduitosError,
};

const BINARY: &str = "conduitos-loongarch64-a4";
const OBSERVATORY_PREFIX: &str = "CONDUIT_OBSERVATORY_SNAPSHOT ";
const PROFILE: &str = "qemu-loongarch64-virt-single-cpu-2g-edk2";

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
    first_kernel: loongarch64_a3::KernelSign,
    second_kernel: loongarch64_a3::KernelSign,
    first_identity: loongarch64_a3::IdentitySign,
    second_identity: loongarch64_a3::IdentitySign,
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
    let paths = Paths::new(ConduitosArch::Loongarch64)?;
    loongarch64_a3::build_variant(opts, BINARY, "loongarch64-a4")?;
    image::assemble(ConduitosArch::Loongarch64, opts)?;
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
    loongarch64_a2::prove(opts)?;
    loongarch64_a3::prove(opts)?;
    let paths = Paths::new(ConduitosArch::Loongarch64)?;
    loongarch64_a3::build_variant(opts, BINARY, "loongarch64-a4")?;
    let image1 = image::assemble(ConduitosArch::Loongarch64, opts)?;
    loongarch64_a3::build_variant(opts, BINARY, "loongarch64-a4")?;
    let image2 = image::assemble(ConduitosArch::Loongarch64, opts)?;
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
    let (qemu, _) = loongarch64_a1::tools(&paths)?;
    let version = Command::new(qemu)
        .arg("--version")
        .output()
        .map_err(|e| refusal("unavailable-loongarch64-emulator", e.to_string()))?;
    let proof = Proof {
        schema: "conduit.conduitos.loongarch64-a4-proof/v1",
        proof_class: "freestanding-loongarch64-ordinary-observatory-native-patchbay",
        base_commit: git_head(&paths.root)?,
        architecture: "loongarch64",
        rust_target: loongarch64_a0::TARGET,
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
        println!("ConduitOS LoongArch64 A4 proof: {}", path.display());
    }
    Ok(())
}

fn boot_once(
    paths: &Paths,
) -> Result<
    (
        loongarch64_a3::KernelSign,
        loongarch64_a3::IdentitySign,
        conduit_observatory::ObservatorySnapshot,
    ),
    ConduitosError,
> {
    let text = loongarch64_a3::boot_transcript(paths)?;
    let entry = loongarch64_a1::parse(&text)?;
    loongarch64_a1::validate(&entry, paths)?;
    let kernel = loongarch64_a3::parse_one(&text, "CONDUIT_KERNEL_SIGN ", "kernel")?;
    let identity =
        loongarch64_a3::parse_one(&text, "CONDUIT_LOONGARCH64_A3_IDENTITY ", "identity")?;
    loongarch64_a3::validate(&kernel, &identity, paths, true)?;
    let snapshot: conduit_observatory::ObservatorySnapshot =
        loongarch64_a3::parse_one(&text, OBSERVATORY_PREFIX, "Observatory")?;
    conduit_observatory::validate_snapshot(&snapshot)
        .map_err(|e| refusal("invalid-loongarch64-observatory-snapshot", e.to_string()))?;
    validate_correlation(&kernel, &snapshot)?;
    Ok((kernel, identity, snapshot))
}

fn validate_correlation(
    kernel: &loongarch64_a3::KernelSign,
    snapshot: &conduit_observatory::ObservatorySnapshot,
) -> Result<(), ConduitosError> {
    let host = snapshot
        .hosts
        .first()
        .ok_or_else(|| refusal("broken-loongarch64-observatory-correlation", "host absent"))?;
    let plan = snapshot
        .plans
        .first()
        .ok_or_else(|| refusal("broken-loongarch64-observatory-correlation", "plan absent"))?;
    let play = snapshot
        .plays
        .first()
        .ok_or_else(|| refusal("broken-loongarch64-observatory-correlation", "play absent"))?;
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
            "broken-loongarch64-observatory-correlation",
            "snapshot does not exactly correlate Host/Boot/Base/Plan/Play truth",
        ));
    }
    Ok(())
}

fn prove_native_patchbay(
    paths: &Paths,
    snapshot: &std::path::Path,
    kernel: &loongarch64_a3::KernelSign,
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
        "patchbay-rejected-loongarch64-snapshot",
    )?;
    let linear = String::from_utf8(output.stdout)
        .map_err(|e| refusal("patchbay-rejected-loongarch64-snapshot", e.to_string()))?;
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
        "firmware=uefi64",
        "proof=FreestandingEmulator",
    ] {
        if !linear.contains(required) {
            return Err(refusal(
                "patchbay-loongarch64-projection-incomplete",
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
            "LoongArch64 A4 requires emulator and native Patchbay execution",
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
