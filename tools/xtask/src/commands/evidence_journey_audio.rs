//! Explicit offline documentary voicing; never a claim of runtime audio playback.
use clap::Args;
use std::{path::PathBuf, process::Command};

#[derive(Args, Debug)]
pub(super) struct JourneyAudioArgs {
    #[arg(long)]
    track: PathBuf,
    #[arg(long)]
    piper: PathBuf,
    #[arg(long)]
    model: PathBuf,
    #[arg(long)]
    config: PathBuf,
}

pub(super) fn run(args: JourneyAudioArgs) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("node")
        .arg(crate::workspace::workspace_root()?.join("proof/browser/journey-audio.mjs"))
        .arg(args.track)
        .arg(args.piper)
        .arg(args.model)
        .arg(args.config)
        .status()?;
    if !status.success() {
        return Err("documentary speech rendering failed".into());
    }
    Ok(())
}
