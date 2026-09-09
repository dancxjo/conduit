//! Canonical live-image proof, separate from the architecture-proof appliance.
use super::{
    journey_proof, live_media, profile::Paths, report::sha256_file, ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use std::fs;

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "graphical-proof-requires-boot",
            "the graphical quality floor requires the actual canonical live image",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let source_commit = clean_head(&paths.root)?;
    live_media::build(live_media::LiveHost::X86_64, opts)?;
    fs::create_dir_all(&paths.target).map_err(io_error)?;
    let image = paths
        .root
        .join("target/conduitos/live/x86_64-pc/conduitos-x86_64.iso");
    let digest = sha256_file(&image)?;
    journey_proof::execute_supplied(opts, &image, digest.clone())?;
    let serial = fs::read_to_string(paths.target.join("journey-serial.log")).map_err(io_error)?;
    let profiles = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_GRAPHICAL_PROFILE "))
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            ConduitosError::refusal("graphical-profile-record-invalid", error.to_string())
        })?;
    if profiles.len() != 1
        || profiles[0]["profile"] != "conduitos/graphical/dejavu-v1"
        || profiles[0]["primitive"] != false
    {
        return Err(ConduitosError::refusal(
            "graphical-profile-not-active",
            "canonical successful boot must use exactly one default graphical profile",
        ));
    }
    let headless = super::graphical_asset_proof::prove(&paths.root, &image, opts)?;
    if clean_head(&paths.root)? != source_commit {
        return Err(ConduitosError::refusal(
            "graphical-proof-source-changed",
            "source changed while building or proving the product images",
        ));
    }
    let proof = serde_json::json!({"schema":"conduit.conduitos/graphical-profile-proof@1", "proof_class":"freestanding-emulator", "source_commit":source_commit, "image_sha256":digest, "profile":profiles[0], "journey":"journey-proof.json", "physical_evidence":false, "headless_images":headless});
    fs::write(
        paths.target.join("graphical-profile-proof.json"),
        serde_json::to_vec_pretty(&proof).map_err(|error| {
            ConduitosError::refusal("graphical-profile-proof-invalid", error.to_string())
        })?,
    )
    .map_err(io_error)?;
    let gallery = r#"<!doctype html><meta charset="utf-8"><title>ConduitOS default graphical profile</title><h1>Default graphical profile</h1><p>Development image · freestanding emulator evidence. These are ordinary shell surfaces from the canonical live ISO, not physical or stable acceptance.</p><p><a href="graphical-profile-proof.json">Profile and image proof</a> · <a href="journey-frames/manifest.json">Exact screenshot provenance</a></p><h2>First boot</h2><img width="960" src="journey-frames/front-door-ready.png" alt="Default Crèche typography and focused name field"><h2>Tour and code</h2><img width="960" src="journey-frames/tour-patchbay-open.png" alt="Tour prose, source code, and native graph"><h2>Hover and selection</h2><img width="960" src="journey-frames/pointer-selected.png" alt="Selected Gear and retained Inspector"><h2>Focused Inspector</h2><img width="960" src="journey-frames/inspector-focused.png" alt="Inspector fields with keyboard focus">"#;
    fs::write(paths.target.join("graphical-profile-gallery.html"), gallery).map_err(io_error)?;
    if !opts.quiet {
        println!(
            "Canonical graphical profile and shell gallery verified: {}",
            paths.target.display()
        );
    }
    Ok(())
}

fn io_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("graphical-profile-evidence-unavailable", error.to_string())
}

fn clean_head(root: &std::path::Path) -> Result<String, ConduitosError> {
    let status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .map_err(io_error)?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err(ConduitosError::refusal(
            "graphical-proof-source-not-clean",
            "commit the graphical profile before collecting exact image evidence",
        ));
    }
    super::report::git_head(root)
}
