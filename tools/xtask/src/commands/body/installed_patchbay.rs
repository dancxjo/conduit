//! Run the checkout-free installed Patchbay proof against supplied products.
use crate::cli::GlobalOpts;
use clap::Args;
use std::{fs, path::PathBuf, process::Command};

#[derive(Args, Debug)]
pub(super) struct ProofArgs {
    /// Reviewed Linux install-bundle directory.
    #[arg(long)]
    bundle: PathBuf,
    /// Built static Handbook directory.
    #[arg(long)]
    handbook: PathBuf,
    /// New directory for the Chromium proof and its receipts.
    #[arg(long)]
    output: PathBuf,
}

pub(super) fn run(args: ProofArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err("installed Patchbay proof requires a Linux x86-64 Host".into());
    }
    let root = crate::workspace::workspace_root()?;
    let bundle = args.bundle.canonicalize()?;
    let handbook = args.handbook.canonicalize()?;
    let output = std::path::absolute(args.output)?;
    let cli = root.join("proof/browser/node_modules/@playwright/test/cli.js");
    if output.exists() {
        return Err("installed Patchbay proof output already exists".into());
    }
    if !bundle.join("hosted-linux-x86_64.json").is_file()
        || !handbook.join("application.application.json").is_file()
        || !cli.is_file()
    {
        return Err(
            "requires a Linux install bundle, static Handbook and pinned Playwright".into(),
        );
    }
    if opts.dry_run {
        println!("would prove installed Patchbay into {}", output.display());
        return Ok(());
    }
    fs::create_dir(&output)?;
    let status = Command::new("node")
        .current_dir(&root)
        .arg(cli)
        .args([
            "test",
            "--config",
            "proof/browser/installed-patchbay.config.mjs",
            "--project",
            "chromium",
            "--output",
        ])
        .arg(output.join("results"))
        .env("CONDUIT_PATCHBAY_INSTALL_BUNDLE", bundle)
        .env("CONDUIT_PATCHBAY_HANDBOOK", handbook)
        .env("PLAYWRIGHT_JSON_OUTPUT_FILE", output.join("report.json"))
        .status()?;
    if !status.success() {
        return Err(format!("installed Patchbay proof failed with {status}").into());
    }
    Ok(())
}
