//! The normal ConduitOS product compiled for VideoCore direct-kernel boot.
use super::{
    armv6_rpi_b_plus_a0,
    profile::Paths,
    report::{git_head, sha256_file, ArtifactRole, BuildRecord},
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use std::{fs, path::Path, process::Command};

pub(super) fn execute(
    opts: &GlobalOpts,
    make: &Path,
    build_id: &str,
    image_id: &str,
    features: &[&str],
) -> Result<BuildRecord, ConduitosError> {
    let paths = Paths::new(ConduitosArch::Armv6)?;
    if !opts.dry_run {
        let mut command = Command::new("cargo");
        command
            .current_dir(&paths.root)
            .args([
                "build",
                "-Zbuild-std=core,alloc",
                "-p",
                "conduitos",
                "--bin",
                "conduitos-armv6-product",
                "--features",
                &features.join(","),
                "--target",
                "armv6-none-eabi",
                "--release",
            ])
            .env("RUSTC_BOOTSTRAP", "1")
            .env("CARGO_PROFILE_RELEASE_STRIP", "debuginfo")
            .env("CONDUITOS_MAKE_RECORD", make)
            .env("CONDUITOS_BUILD_ID", build_id)
            .env("CONDUITOS_IMAGE_ID", image_id)
            .env(
                "RUSTFLAGS",
                "-C relocation-model=static -C panic=abort -C opt-level=3 -C codegen-units=1",
            );
        if opts.locked {
            command.arg("--locked");
        }
        let status = command
            .status()
            .map_err(|e| ConduitosError::refusal("armv6-product-build-failed", e.to_string()))?;
        if !status.success() {
            return Err(ConduitosError::refusal(
                "armv6-product-build-failed",
                status.to_string(),
            ));
        }
        let built = paths.built_binary("armv6-none-eabi", "conduitos-armv6-product");
        fs::copy(&built, &paths.kernel).map_err(|e| {
            ConduitosError::refusal("armv6-product-artifact-unavailable", e.to_string())
        })?;
        armv6_rpi_b_plus_a0::inspect_symbol(&paths.kernel, &paths.root)?;
        let elf = fs::read(&paths.kernel).map_err(|e| {
            ConduitosError::refusal("armv6-product-artifact-unavailable", e.to_string())
        })?;
        let image = armv6_rpi_b_plus_a0::product_load_image(&elf)?;
        fs::write(paths.target.join("kernel.img"), image).map_err(|e| {
            ConduitosError::refusal("armv6-product-image-write-failed", e.to_string())
        })?;
    }
    Ok(BuildRecord {
        schema: "conduit.conduitos.build/v2",
        artifact_role: ArtifactRole::ProductHost,
        base_commit: git_head(&paths.root)?,
        architecture: "armv6",
        rust_target: "armv6-none-eabi",
        limine_crate: "none",
        elf_sha256: if opts.dry_run {
            "dry-run".into()
        } else {
            sha256_file(&paths.kernel)?
        },
    })
}
