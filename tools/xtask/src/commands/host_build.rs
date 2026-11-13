//! Resolve and fabricate one exact Host image from explicit captured inputs.
use super::{
    command_identity, host_esp32_native, host_rp2040, host_target, host_workset_source,
    resolve_profile,
};
use crate::cli::GlobalOpts;
use std::{fs, path::PathBuf};

pub(super) struct Input {
    pub profile: PathBuf,
    pub output: PathBuf,
    pub source_identity: Option<String>,
    pub workset_source: Option<PathBuf>,
    pub workset_entry: Vec<String>,
    pub workset_specializations: Option<PathBuf>,
    pub private_seed_input: Option<PathBuf>,
    pub private_seed_output: Option<PathBuf>,
}
pub(super) fn run(input: Input, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let Input {
        profile: profile_path,
        output,
        source_identity,
        workset_source,
        workset_entry,
        workset_specializations,
        private_seed_input,
        private_seed_output,
    } = input;
    let source_identity = source_identity
        .map(Ok)
        .unwrap_or_else(|| command_identity("git", &["rev-parse", "HEAD"]))?;
    let workset = workset_source
        .as_deref()
        .map(|path| {
            host_workset_source::WorksetSourceInput::capture(
                path,
                &workset_entry,
                workset_specializations.as_deref(),
                &source_identity,
                &output,
                opts.dry_run,
            )
        })
        .transpose()?;
    let resolved_identity = workset
        .as_ref()
        .map_or(source_identity.as_str(), |input| input.identity.as_str());
    let (image, bytes) = resolve_profile(&profile_path, resolved_identity.to_owned())?;
    if opts.dry_run {
        println!(
            "would BUILD {} from resolved binding {}",
            profile_path.display(),
            image.manifest.image_id
        );
    }
    if host_esp32_native::selected(&image) {
        host_esp32_native::build(
            &profile_path,
            &output,
            &image,
            &bytes,
            private_seed_input.as_deref(),
            private_seed_output.as_deref(),
            workset.as_ref(),
            opts,
        )?;
        return Ok(());
    }
    if image.manifest.target == conduit_host_rp2040::TARGET_ID {
        host_rp2040::build(
            &profile_path,
            &output,
            &image,
            &bytes,
            private_seed_input.as_deref(),
            private_seed_output.as_deref(),
            workset.as_ref(),
            opts,
        )?;
        return Ok(());
    }
    if workset.is_some() {
        return Err("workset Source specialization is not supported by this target adapter".into());
    }
    if private_seed_input.is_some() || private_seed_output.is_some() {
        return Err("private embedded Seed applies only to an RP2040 Host build".into());
    }
    if crate::commands::conduitos::target_backend::find(&image.manifest.target).is_some() {
        let target = host_target::build_target(&image, &bytes, &output, opts)?;
        if opts.json {
            println!("{}", serde_json::to_string(&target)?);
        } else if !opts.quiet {
            println!("BUILT {} ({:?})", target.image_id, image.manifest.output);
            println!("IMAGE: {}", output.join(&target.image.file).display());
            println!("manifest: {}", output.join("build-manifest.json").display());
        }
    } else if !opts.dry_run {
        fs::create_dir_all(&output)?;
        fs::write(output.join("image.json"), &bytes)?;
        fs::write(
            output.join("build-manifest.json"),
            serde_json::to_vec_pretty(&image.manifest)?,
        )?;
        if opts.json {
            println!("{}", serde_json::to_string(&image.manifest)?);
        } else if !opts.quiet {
            println!(
                "BUILT {} ({:?})",
                image.manifest.image_id, image.manifest.output
            );
            println!("IMAGE: {}", output.join("image.json").display());
            println!("manifest: {}", output.join("build-manifest.json").display());
        }
    }
    Ok(())
}
