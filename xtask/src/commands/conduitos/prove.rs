use std::fs;

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION, QEMU_PROFILE},
    report::{git_head, ProofRecord},
    run, ConduitosArch, ConduitosError,
};

pub fn execute(arch: ConduitosArch, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-proof",
            "prove --dry-run cannot manufacture QEMU evidence",
        ));
    }
    let paths = Paths::new(arch)?;
    let image = image::execute(arch, opts)?;
    let rebuilt_image = image::execute(arch, opts)?;
    let reproducible_image = image.iso_sha256 == rebuilt_image.iso_sha256;
    if !reproducible_image {
        return Err(ConduitosError::refusal(
            "non-reproducible-image",
            format!(
                "identical inputs produced {} then {}",
                image.iso_sha256, rebuilt_image.iso_sha256
            ),
        ));
    }
    let first = run::boot_once(&paths, opts)?;
    let second = run::boot_once(&paths, opts)?;
    let fresh_host_id = first.boot.host_id != second.boot.host_id;
    let fresh_boot_id = first.boot.boot_id != second.boot.boot_id;
    if !fresh_host_id || !fresh_boot_id {
        return Err(ConduitosError::refusal(
            "stale-boot-identity",
            "two independent QEMU boots reused HostId or BootId",
        ));
    }
    let stable_semantic_identities = first.kernel.source_document_id
        == second.kernel.source_document_id
        && first.kernel.checked_form_id == second.kernel.checked_form_id
        && first.kernel.expanded_form_id == second.kernel.expanded_form_id;
    let fresh_realization_identities = first.kernel.plan_id != second.kernel.plan_id
        && first.kernel.fragment_id != second.kernel.fragment_id
        && first.kernel.active_play_id != second.kernel.active_play_id;
    if !stable_semantic_identities || !fresh_realization_identities {
        return Err(ConduitosError::refusal(
            "observatory-identity-stage-collapse",
            "independent boots did not preserve semantic identities and refresh realization identities",
        ));
    }
    let base_commit = git_head(&paths.root)?;
    let expected_image_id = format!("conduitos-image/{base_commit}/{}/v1", arch.as_str());
    if first.boot.build_id != base_commit
        || second.boot.build_id != base_commit
        || first.boot.image_id != expected_image_id
        || second.boot.image_id != expected_image_id
    {
        return Err(ConduitosError::refusal(
            "stale-build-identity",
            "guest build/image identity did not match the exact checkout",
        ));
    }
    let qemu_version = qemu_version(&paths)?;
    let mut proof = ProofRecord {
        schema: "conduit.conduitos.observatory-proof/v1",
        base_commit,
        architecture: arch.as_str(),
        proof_class: "freestanding-emulator",
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        qemu_profile: QEMU_PROFILE,
        qemu_version,
        iso_sha256: image.iso_sha256,
        reproducible_image,
        first_boot: first.boot,
        first_kernel: first.kernel,
        first_observatory: first.observatory.clone(),
        second_boot: second.boot,
        second_kernel: second.kernel,
        second_observatory: second.observatory,
        fresh_host_id,
        fresh_boot_id,
        stable_semantic_identities,
        fresh_realization_identities,
        native_patchbay_consumed: false,
        native_patchbay_linear_lines: 0,
    };
    let snapshot = serde_json::to_vec_pretty(&first.observatory)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    fs::write(&paths.observatory_snapshot, snapshot)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    proof.native_patchbay_linear_lines = prove_native_patchbay(&paths, &proof)?;
    proof.native_patchbay_consumed = true;
    let bytes = serde_json::to_vec_pretty(&proof)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    fs::write(&paths.proof, bytes)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string(&proof).map_err(|error| {
                ConduitosError::refusal("proof-record-failed", error.to_string())
            })?
        );
    } else if !opts.quiet {
        println!(
            "ConduitOS P5 Observatory/Patchbay proof: {}\nConduitOS Observatory snapshot: {}",
            paths.proof.display(),
            paths.observatory_snapshot.display()
        );
    }
    Ok(())
}

fn prove_native_patchbay(paths: &Paths, proof: &ProofRecord) -> Result<usize, ConduitosError> {
    let snapshot_path = paths
        .observatory_snapshot
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
            snapshot_path,
        ],
        &paths.root,
        "patchbay-rejected-report",
    )?;
    let linear = String::from_utf8(output.stdout)
        .map_err(|error| ConduitosError::refusal("patchbay-rejected-report", error.to_string()))?;
    for required in [
        proof.first_boot.host_id.as_str(),
        proof.first_boot.boot_id.as_str(),
        proof.first_kernel.plan_id.as_str(),
        proof.first_kernel.fragment_id.as_str(),
        proof.first_kernel.active_play_id.as_str(),
        proof.first_kernel.source_document_id.as_str(),
        proof.first_kernel.checked_form_id.as_str(),
        proof.first_kernel.expanded_form_id.as_str(),
        "BASES 7",
        "SIGNS 13",
        "items=1 bytes=256",
        "implementation=conduitos/kernel-text-literal@1",
        "implementation=conduitos/kernel-text-upper@1",
        "implementation=conduitos/kernel-serial-text@1",
        "profile=conduitos/cooperative-bounded-step@1",
        "runtime-memory=12288",
        "timer-slots=0",
        "lifecycle=Completed",
        "visible_gaps=0",
        "history=current",
        "history=historical",
        "BOOT PROVENANCE [SEALED] 1",
        "proof=FreestandingEmulator",
    ] {
        if !linear.contains(required) {
            return Err(ConduitosError::refusal(
                "patchbay-linear-projection-incomplete",
                format!("native Patchbay output omitted {required}"),
            ));
        }
    }
    Ok(linear.lines().count())
}

fn qemu_version(paths: &Paths) -> Result<String, ConduitosError> {
    let output = super::profile::command(
        "qemu-system-x86_64",
        &["--version"],
        &paths.root,
        "missing-qemu",
    )?;
    String::from_utf8(output.stdout)
        .map(|value| value.lines().next().unwrap_or_default().to_owned())
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))
}
