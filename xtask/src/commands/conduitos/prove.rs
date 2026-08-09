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
    let fresh_host_id = first.host_id != second.host_id;
    let fresh_boot_id = first.boot_id != second.boot_id;
    if !fresh_host_id || !fresh_boot_id {
        return Err(ConduitosError::refusal(
            "stale-boot-identity",
            "two independent QEMU boots reused HostId or BootId",
        ));
    }
    let base_commit = git_head(&paths.root)?;
    let expected_image_id = format!("conduitos-image/{base_commit}/{}/v1", arch.as_str());
    if first.build_id != base_commit
        || second.build_id != base_commit
        || first.image_id != expected_image_id
        || second.image_id != expected_image_id
    {
        return Err(ConduitosError::refusal(
            "stale-build-identity",
            "guest build/image identity did not match the exact checkout",
        ));
    }
    let qemu_version = qemu_version(&paths)?;
    let proof = ProofRecord {
        schema: "conduit.conduitos.boot-proof/v1",
        base_commit,
        architecture: arch.as_str(),
        proof_class: "freestanding-emulator",
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        qemu_profile: QEMU_PROFILE,
        qemu_version,
        iso_sha256: image.iso_sha256,
        reproducible_image,
        first_boot: first,
        second_boot: second,
        fresh_host_id,
        fresh_boot_id,
    };
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
        println!("ConduitOS P0/P1 proof: {}", paths.proof.display());
    }
    Ok(())
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
