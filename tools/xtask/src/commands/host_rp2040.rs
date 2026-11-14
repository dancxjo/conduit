//! Actual generic Pico Host build. Public code and private native Seed stay separate.
use crate::cli::GlobalOpts;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command};

use super::host_rp2040_membership_seed::compose_private_seed;

pub(super) fn build(
    input: super::host_build::EmbeddedInput<'_>,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let super::host_build::EmbeddedInput {
        profile,
        output,
        image,
        description: description_bytes,
        private_input,
        private_output,
        workset,
    } = input;
    let checked = super::load_configuration(profile)?;
    conduit_host_rp2040::check_environmental_construction(&checked)?;
    conduit_host_rp2040::retained_wifi_selected(&checked)?;
    if opts.dry_run {
        return Ok(());
    }
    let root = crate::workspace::workspace_root()?;
    let state = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(&root)
        .output()?;
    if !state.status.success() || !state.stdout.is_empty() {
        return Err("RP2040 firmware requires clean committed Source".into());
    }
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()?;
    if !head.status.success()
        || String::from_utf8(head.stdout)?.trim()
            != workset.map_or(image.manifest.source_identity.as_str(), |input| {
                input.git_revision.as_str()
            })
    {
        return Err("RP2040 build source identity differs from the exact committed tree".into());
    }
    let manifest = root.join("targets/rp2040/firmware/host/Cargo.toml");
    let profile = profile.canonicalize()?;
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/rp2040-host"));
    let target_dir = if target_dir.is_absolute() {
        target_dir
    } else {
        root.join(target_dir)
    };
    let description = super::host_workset_source::ImageDescriptionInput::capture(
        output,
        description_bytes,
        image,
    )?;
    let mut command = Command::new("cargo");
    let command = command
        .current_dir(&root)
        .args([
            "+stable",
            "build",
            "--locked",
            "--release",
            "--target",
            "thumbv6m-none-eabi",
            "--manifest-path",
        ])
        .arg(&manifest)
        .env("CONDUIT_SOURCE_IDENTITY", &image.manifest.source_identity)
        .env("CONDUIT_HOST_IMAGE_ID", &image.manifest.image_id)
        .env("CONDUIT_HOST_CONFIGURATION", profile)
        .env("CARGO_TARGET_DIR", &target_dir);
    super::host_workset_source::clear_inputs(command);
    description.configure(command)?;
    if let Some(input) = workset {
        input.configure(command)?;
    }
    let build_out = super::host_cargo_build::run(command, &target_dir, "conduit-rp2040-host")?;
    let post = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(&root)
        .output()?;
    let post_head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()?;
    if !post.status.success()
        || !post.stdout.is_empty()
        || !post_head.status.success()
        || String::from_utf8(post_head.stdout)?.trim()
            != workset.map_or(image.manifest.source_identity.as_str(), |input| {
                input.git_revision.as_str()
            })
    {
        return Err("RP2040 Source changed during compilation".into());
    }
    fs::create_dir_all(output)?;
    let elf = target_dir.join("thumbv6m-none-eabi/release/conduit-rp2040-host");
    let uf2 = output.join("host.uf2");
    let status = crate::process::command_for("elf2uf2-rs")
        .arg(&elf)
        .arg(&uf2)
        .status()?;
    if !status.success() {
        return Err("RP2040 UF2 conversion failed".into());
    }
    description.verify()?;
    let bytes = fs::read(&uf2)?;
    validate_application_uf2(&bytes)?;
    // Feed the existing verified artifact/package path used by Crèche. This
    // inventory describes inert bytes; it grants no deployment or membership.
    if let Some(input) = workset {
        input.verify()?;
    }
    let mut payloads = vec![("host.uf2", "application/x-uf2")];
    if super::host_continuation_recipe::publish(
        &build_out,
        output,
        image,
        checked.configuration_id(),
        workset,
    )? {
        payloads.push((super::host_continuation_recipe::NAME, "application/json"));
    }
    super::host_release::seal(
        output,
        "checked-artifact.json",
        &image.manifest.target,
        &image.manifest.make_package_id,
        "uf2",
        &image.manifest.builder_adapter,
        image
            .manifest
            .deployment_adapter
            .as_deref()
            .ok_or("RP2040 deployment adapter absent")?,
        &image.manifest.source_identity,
        &payloads,
    )?;
    // Use the existing target-owned release extension; no extra payload or
    // competing manifest is needed to select the matching native Seed codec.
    let artifact_path = output.join("checked-artifact.json");
    let mut artifact: conduit_host_make::HostReleaseManifest =
        serde_json::from_slice(&fs::read(&artifact_path)?)?;
    artifact.target_metadata.insert(
        conduit_host_rp2040::BOOTSTRAP_CONTRACT_METADATA.into(),
        serde_json::to_value(conduit_host_rp2040::BootstrapContract::for_image(
            &image.manifest.image_id,
            checked.configuration_id(),
        ))?,
    );
    fs::write(artifact_path, serde_json::to_vec_pretty(&artifact)?)?;
    if let Some(input) = workset {
        input.publish(&output.join("checked-artifact.json"))?;
    }
    let elf_digest = format!("{:x}", Sha256::digest(fs::read(&elf)?));
    let uf2_digest = format!("{:x}", Sha256::digest(&bytes));
    // Retain the exact ordinary Make result used to bind this firmware and Seed.
    // The public description contains no private invitation or member key.
    fs::write(output.join("resolved-image.json"), description_bytes)?;
    fs::write(
        output.join("build-manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema":"conduit.rp2040-host/build@1","source_identity":image.manifest.source_identity,
            "configuration_id":checked.configuration_id(),"resolved_description_binding":image.manifest.image_id,
            "target":conduit_host_rp2040::TARGET_ID,"firmware_elf_sha256":elf_digest,
            "application_uf2_sha256":uf2_digest,"body_membership_proven":false,
            "local_sampling_plan_proven":false
        }))?,
    )?;
    if let (Some(input), Some(destination)) = (private_input, private_output) {
        let public = output.canonicalize()?;
        let parent = destination
            .parent()
            .ok_or("private Seed destination requires parent")?
            .canonicalize()?;
        if parent.starts_with(&public) || input.canonicalize()?.starts_with(&public) {
            return Err(
                "private Seed input/output must stay outside the public artifact directory".into(),
            );
        }
        let basis = compose_private_seed(&checked, &image.manifest.image_id, input, destination)?;
        basis.publish(output)?;
    }
    if !opts.quiet {
        println!("RP2040 application: {}", uf2.display());
    }
    Ok(())
}
pub(super) fn validate_application_uf2(bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use conduit_rp2040_flash_layout::{
        classify_image_page, ImageRegion, Uf2Validator, UF2_BLOCK_BYTES,
    };
    let mut validator =
        Uf2Validator::new(bytes.len() as u64).map_err(|e| format!("UF2 extent: {e:?}"))?;
    for block in bytes.as_chunks::<UF2_BLOCK_BYTES>().0 {
        validator
            .push(block)
            .map_err(|e| format!("UF2 validation: {e:?}"))?;
        let address = u32::from_le_bytes(block[12..16].try_into()?);
        let length = u32::from_le_bytes(block[16..20].try_into()?);
        if classify_image_page(address, length) != Ok(ImageRegion::Firmware) {
            return Err("public application reaches private reserved flash".into());
        }
    }
    validator
        .finish()
        .map_err(|e| format!("UF2 incomplete: {e:?}"))?;
    Ok(())
}
