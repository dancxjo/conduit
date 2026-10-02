//! Proof orchestration over retained owner and canonical Browser SDK artifacts.
use crate::cli::GlobalOpts;
use clap::Args;
use std::{path::PathBuf, process::Command};

#[derive(Args, Debug)]
pub(super) struct ProofArgs {
    /// Exact product executable to install and exercise; its bytes are hashed in the receipt.
    #[arg(long)]
    owner: PathBuf,
    /// SDK produced by `make host browser-sdk-package`; its guest source is retained.
    #[arg(long)]
    sdk: PathBuf,
    /// New evidence directory; existing evidence is never overwritten.
    #[arg(long)]
    output: PathBuf,
}

pub(super) fn run(args: ProofArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if args.output.exists() {
        return Err("browser admission evidence output already exists".into());
    }
    let root = crate::workspace::workspace_root()?;
    let owner = args.owner.canonicalize()?;
    let sdk = args.sdk.canonicalize()?;
    let playwright = root.join("proof/browser/node_modules/playwright/index.mjs");
    if !owner.is_file() || !sdk.join("browser-sdk.mjs").is_file() || !playwright.is_file() {
        return Err("requires an owner executable, packaged browser SDK and pinned browser proof dependencies".into());
    }
    if opts.dry_run {
        println!(
            "would prove browser admission with {} and {} into {}",
            owner.display(),
            sdk.display(),
            args.output.display()
        );
        return Ok(());
    }
    let status = Command::new("node")
        .arg(root.join("products/conduit/src/body_owner/participants/browser-proof.mjs"))
        .arg(owner)
        .arg(sdk)
        .arg(args.output)
        .arg(playwright)
        .status()?;
    if !status.success() {
        return Err(format!("browser admission proof failed with {status}").into());
    }
    Ok(())
}
