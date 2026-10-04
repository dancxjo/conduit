//! Correlate native and browser product proofs without constructing their state.

use std::{path::PathBuf, process::Command};

use clap::Args as ClapArgs;

use crate::cli::GlobalOpts;

use super::{profile::Paths, ConduitosArch, ConduitosError};

#[derive(ClapArgs, Debug, Clone)]
pub(super) struct Args {
    /// Product executable named by the installed Linux Host release.
    #[arg(long)]
    owner: PathBuf,
    /// State directory of that installed, running Body owner.
    #[arg(long)]
    owner_state: PathBuf,
    /// Exact-source static Handbook package.
    #[arg(long)]
    handbook: PathBuf,
    /// Private ISO provisioned for that Body and owner route.
    #[arg(long)]
    spore: PathBuf,
    /// Routed candidate ID contained in that ISO.
    #[arg(long)]
    candidate_id: String,
    /// Explicit private IPv4 TLS owner listener.
    #[arg(long)]
    owner_forward: std::net::SocketAddr,
    /// New private directory for correlated browser and QMP evidence.
    #[arg(long)]
    output_dir: PathBuf,
    /// Pinned Playwright module installed for the browser proof project.
    #[arg(long)]
    playwright: PathBuf,
}

pub(super) fn execute(args: &Args, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "three-host-proof-requires-live-input",
            "the three-host proof must launch the real browser and QMP guest",
        ));
    }
    let root = Paths::new(ConduitosArch::X86_64)?.root;
    let script = root.join("proof/browser/three-host-owner-journey.mjs");
    for (name, path) in [
        ("script", script.clone()),
        ("owner", args.owner.clone()),
        (
            "handbook",
            args.handbook.join("application.application.json"),
        ),
        ("spore", args.spore.clone()),
        ("playwright", args.playwright.clone()),
        ("installation", args.owner_state.join("installation.json")),
    ] {
        if !path.is_file() {
            return Err(ConduitosError::refusal(
                "three-host-proof-prerequisite",
                format!("{name} is unavailable: {}", path.display()),
            ));
        }
    }
    if args.output_dir.exists() {
        return Err(ConduitosError::refusal(
            "three-host-proof-output-exists",
            "provide a new private evidence directory",
        ));
    }
    let current = std::env::current_exe()
        .map_err(|error| ConduitosError::refusal("three-host-proof-xtask", error.to_string()))?;
    let status = Command::new("node")
        .arg(&script)
        .arg(current)
        .arg(&args.owner)
        .arg(&args.owner_state)
        .arg(&args.handbook)
        .arg(&args.spore)
        .arg(&args.candidate_id)
        .arg(args.owner_forward.to_string())
        .arg(&args.output_dir)
        .arg(&args.playwright)
        .current_dir(&root)
        .status()
        .map_err(|error| ConduitosError::refusal("three-host-proof-launch", error.to_string()))?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "three-host-proof-failed",
            format!("browser and QMP journey exited with {status}"),
        ));
    }
    Ok(())
}
