use std::{path::Path, process::Command};

use crate::cli::GlobalOpts;

pub(super) fn run(
    bundle: &Path,
    output: &Path,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if output.exists() {
        return Err(format!("SDK package output already exists: {}", output.display()).into());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = repository.join("targets/browser/sdk/package-browser-bundle.mjs");
    let status = Command::new("node")
        .arg(script)
        .arg(bundle)
        .arg(output)
        .current_dir(&repository)
        .status()?;
    if !status.success() {
        return Err(format!("Browser SDK package producer failed with {status}").into());
    }
    if !opts.quiet {
        println!("Browser SDK package: {}", output.display());
    }
    Ok(())
}
