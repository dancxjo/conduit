//! Final target-artifact receipt for PROFILE-driven Host BUILDs.

use std::{fs, path::Path};

use conduit_host_fabrication::{verify_image_binding, BuildManifest, HostImage};
use serde::{Deserialize, Serialize};

use crate::{
    cli::GlobalOpts,
    commands::conduitos::{
        build_profile_image, target_backend, target_build::boot_profile_image,
        target_build::verify_artifact_digest, ArtifactRole, ProfileBuiltImage,
    },
};

pub const TARGET_BUILD_MANIFEST_SCHEMA: &str = "conduit.host/target-build-manifest@3";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReceipt {
    pub role: String,
    pub file: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootAssetsReceipt {
    pub packager: String,
    pub limine_version: String,
    pub limine_archive_sha256: String,
    pub architecture: String,
    pub machine: String,
    pub firmware: String,
    pub boot_entry: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetBuildManifest {
    pub schema: String,
    pub artifact_role: ArtifactRole,
    pub profile_id: String,
    pub build_id: String,
    /// The sole canonical identity of the final bootable IMAGE.
    pub image_id: String,
    /// Binds the earlier resolved JSON description without promoting it to a
    /// second final IMAGE identity.
    pub resolved_description_binding: String,
    pub source_identity: String,
    pub toolchain_identity: String,
    pub target: String,
    pub kernel: ArtifactReceipt,
    pub image: ArtifactReceipt,
    pub boot_assets: BootAssetsReceipt,
    pub resolved_build: BuildManifest,
}

pub fn build_target(
    image: &HostImage,
    description_bytes: &[u8],
    output: &Path,
    opts: &GlobalOpts,
) -> Result<TargetBuildManifest, Box<dyn std::error::Error>> {
    let backend = target_backend::select(&image.manifest.target)?;
    let built = build_profile_image(&image.manifest, description_bytes, opts)?;
    if opts.dry_run {
        return Ok(receipt(image, &built));
    }

    fs::create_dir_all(output)?;
    let kernel_name = backend.kernel_file;
    let image_name = backend.image_file;
    fs::copy(&built.kernel, output.join(kernel_name))?;
    fs::copy(&built.image, output.join(image_name))?;
    verify_artifact_digest(&output.join(kernel_name), &built.kernel_sha256)?;
    verify_artifact_digest(&output.join(image_name), &built.image_sha256)?;
    fs::write(output.join("resolved-image.json"), description_bytes)?;
    let manifest = receipt(image, &built);
    fs::write(
        output.join("build-manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(manifest)
}

pub fn verify_target(output: &Path) -> Result<TargetBuildManifest, Box<dyn std::error::Error>> {
    let manifest_bytes = fs::read(output.join("build-manifest.json"))?;
    let manifest: TargetBuildManifest = serde_json::from_slice(&manifest_bytes)?;
    if manifest.schema != TARGET_BUILD_MANIFEST_SCHEMA {
        return Err(format!(
            "unsupported target BUILD manifest schema {}",
            manifest.schema
        )
        .into());
    }
    if manifest.artifact_role != ArtifactRole::ProductHost {
        return Err("target BUILD manifest is not a product Host artifact".into());
    }
    let backend = target_backend::select(&manifest.target)?;
    if manifest.kernel.file != backend.kernel_file
        || manifest.kernel.role != backend.kernel_role
        || manifest.image.file != backend.image_file
        || manifest.image.role != backend.image_role
    {
        return Err(
            "target BUILD artifact names or roles do not match the selected backend".into(),
        );
    }
    verify_file(output, &manifest.kernel)?;
    verify_file(output, &manifest.image)?;
    let expected_image_id = format!("image:sha256:{}", manifest.image.sha256);
    if manifest.image_id != expected_image_id {
        return Err(format!(
            "final ImageId mismatch: expected {expected_image_id}, found {}",
            manifest.image_id
        )
        .into());
    }
    let description_bytes = fs::read(output.join("resolved-image.json"))?;
    let description: HostImage = serde_json::from_slice(&description_bytes)?;
    verify_image_binding(&description, &description_bytes)
        .map_err(|diagnostic| format!("resolved BUILD description mismatch: {diagnostic:?}"))?;
    if description.manifest != manifest.resolved_build
        || manifest.resolved_description_binding != description.manifest.image_id
        || manifest.profile_id != description.manifest.profile_id
        || manifest.build_id != description.manifest.build_id
        || manifest.target != description.manifest.target
    {
        return Err("target BUILD manifest does not bind its resolved PROFILE closure".into());
    }
    let expected_boot = boot_assets(backend, &manifest.boot_assets);
    if manifest.boot_assets != expected_boot {
        return Err(
            "target BUILD manifest has the wrong architecture, machine, or EFI binding".into(),
        );
    }
    Ok(manifest)
}

pub fn boot_target(
    output: &Path,
    manifest: &TargetBuildManifest,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    boot_profile_image(
        &output.join(&manifest.image.file),
        &manifest.target,
        &manifest.profile_id,
        &manifest.build_id,
        &manifest.resolved_description_binding,
        opts,
    )?;
    Ok(())
}

fn verify_file(
    output: &Path,
    artifact: &ArtifactReceipt,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = output.join(&artifact.file);
    verify_artifact_digest(&path, &artifact.sha256)
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
}

fn receipt(image: &HostImage, built: &ProfileBuiltImage) -> TargetBuildManifest {
    let backend = target_backend::select(&image.manifest.target)
        .expect("receipt target was checked before lowering");
    let final_image_id = format!("image:sha256:{}", built.image_sha256);
    TargetBuildManifest {
        schema: TARGET_BUILD_MANIFEST_SCHEMA.into(),
        artifact_role: built.artifact_role,
        profile_id: image.manifest.profile_id.clone(),
        build_id: image.manifest.build_id.clone(),
        image_id: final_image_id.clone(),
        resolved_description_binding: image.manifest.image_id.clone(),
        source_identity: image.manifest.source_identity.clone(),
        toolchain_identity: image.manifest.toolchain_identity.clone(),
        target: image.manifest.target.clone(),
        kernel: ArtifactReceipt {
            role: backend.kernel_role.into(),
            file: backend.kernel_file.into(),
            sha256: built.kernel_sha256.clone(),
        },
        image: ArtifactReceipt {
            role: backend.image_role.into(),
            file: backend.image_file.into(),
            sha256: built.image_sha256.clone(),
        },
        boot_assets: boot_assets(
            backend,
            &BootAssetsReceipt {
                packager: "pinned-limine-hybrid-iso".into(),
                limine_version: built.limine_version.into(),
                limine_archive_sha256: built.limine_archive_sha256.into(),
                architecture: String::new(),
                machine: String::new(),
                firmware: String::new(),
                boot_entry: String::new(),
            },
        ),
        resolved_build: image.manifest.clone(),
    }
}

fn boot_assets(
    backend: &target_backend::TargetBackend,
    source: &BootAssetsReceipt,
) -> BootAssetsReceipt {
    BootAssetsReceipt {
        packager: source.packager.clone(),
        limine_version: source.limine_version.clone(),
        limine_archive_sha256: source.limine_archive_sha256.clone(),
        architecture: backend.architecture.into(),
        machine: backend.machine.into(),
        firmware: backend.firmware.into(),
        boot_entry: backend.boot_entry.into(),
    }
}
