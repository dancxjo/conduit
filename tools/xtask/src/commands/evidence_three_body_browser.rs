//! Human-facing publication checks in the repository's pinned Chromium.
use clap::Args;
use std::{path::PathBuf, process::Command};

#[derive(Args, Debug)]
pub(super) struct BrowserArgs {
    #[arg(long)]
    publication_root: PathBuf,
    #[arg(long)]
    output: PathBuf,
}

pub(super) fn run(args: BrowserArgs) -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::workspace::workspace_root()?;
    let status = Command::new("node")
        .current_dir(&root)
        .arg(root.join("proof/browser/verify-three-body-documentary.mjs"))
        .arg(args.publication_root)
        .arg(args.output)
        .status()?;
    if !status.success() {
        return Err("Three Bodies browser documentary check failed".into());
    }
    Ok(())
}
