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
    let gallery = r#"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>A Body, its Patchbay, and its Face · ConduitOS</title><style>body{font:1.05rem/1.6 system-ui,sans-serif;background:#111b24;color:#edf2f5;margin:0 auto;padding:2rem;max-width:72rem}a{color:#9bdaf2}nav,figure{background:#1d2b37;border:1px solid #344a58;border-radius:.7rem;padding:1rem;margin:1.5rem 0}img{display:block;width:100%;height:auto;background:#111}figcaption{margin-top:.8rem}h1,h2{line-height:1.15}small{color:#b8c8d2}</style><nav aria-label="Evidence links"><a href="graphical-profile-proof.json">Graphical profile receipt</a> · <a href="journey-proof.json">Journey receipt</a> · <a href="journey-frames/manifest.json">Screenshot provenance</a></nav><main><h1>A Body, its Patchbay, and its Face</h1><p>These images follow keyboard actions in one real ConduitOS x86 guest. The QMP harness captured the display after each guest checkpoint. This is freestanding emulator evidence from a development image; physical and human enactment remain separate.</p><figure><img src="journey-frames/front-door-ready.png" alt="Crèche at first boot, before any Body exists"><figcaption><h2>1. Arrive</h2>Crèche offers a name and available Plots. No Body exists yet.</figcaption></figure><figure><img src="journey-frames/body-born.png" alt="Newly born Body resting with its chosen Plots"><figcaption><h2>2. Birth</h2>The user activates Birth. The Body is retained and resting; Birth does not silently start a Play.</figcaption></figure><figure><img src="journey-frames/body-woken.png" alt="Body after Wake"><figcaption><h2>3. Wake</h2>Wake admits the current work. Its Plan and Play remain distinct actions.</figcaption></figure><figure><img src="journey-frames/body-planned.png" alt="Body with an exact current Plan"><figcaption><h2>4. Plan</h2>The Host selects exact backs for the Body's Plots, including the gears, ports, and cords visible later in Patchbay.</figcaption></figure><figure><img src="journey-frames/body-playing.png" alt="Body after Play begins"><figcaption><h2>5. Play</h2>The same Body starts its admitted Play and awaits input.</figcaption></figure><figure><img src="journey-frames/home.png" alt="Home navigation after the Body starts"><figcaption><h2>6. Go Home</h2>The user opens Home without losing the Body.</figcaption></figure><figure><img src="journey-frames/patchbay-workspace.png" alt="Patchbay workspace for the current Body"><figcaption><h2>7. Open Patchbay</h2>The resident Patchbay is selected from Home.</figcaption></figure><figure><img src="journey-frames/patchbay-face.png" alt="Semantic Face shown through the native Mask"><figcaption><h2>8. Inspect the Face</h2>F2 presents the current semantic Face through the native Mask.</figcaption></figure><figure><img src="journey-frames/patchbay-diagram.png" alt="Live diagram of the current planned gears, ports, and cords"><figcaption><h2>9. View the diagram</h2>F3 draws the current planned graph from Face subjects and relationships, rather than loading a pre-generated SVG.</figcaption></figure><figure><img src="journey-frames/body-stopped.png" alt="Same Body resting after Stop through the Face"><figcaption><h2>10. Stop</h2>The user requests Stop through the Face. The Body stays identified and can be woken again.</figcaption></figure></main></html>"#;
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
