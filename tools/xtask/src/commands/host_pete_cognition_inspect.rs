//! Read-only historical Patchbay inspection of an actual cognition proof run.
use crate::cli::GlobalOpts;
use std::{error::Error, path::Path, process::Command};
pub(super) fn inspect(
    evidence: &Path,
    browser_proof: bool,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would validate and {} the historical cognition run {} through Patchbay",
                if browser_proof {
                    "browser-prove"
                } else {
                    "serve"
                },
                evidence.display()
            );
        }
        return Ok(());
    }
    let root = crate::workspace::workspace_root()?;
    let evidence = root.join(evidence).canonicalize()?;
    let mut cargo = Command::new("cargo");
    cargo.current_dir(&root).args([
        if browser_proof { "build" } else { "run" },
        "--locked",
        "-p",
        "conduit-browser-patchbay-workbench",
        "--bin",
        "pete-cognition-inspect",
    ]);
    if !browser_proof {
        cargo.arg("--").arg(&evidence);
    }
    if !cargo.status()?.success() {
        return Err("historical cognition inspector build or execution failed".into());
    }
    if browser_proof {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| root.join("target"));
        let binary = root.join(target).join("debug").join(format!(
            "pete-cognition-inspect{}",
            std::env::consts::EXE_SUFFIX
        ));
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();
        let output = evidence.join(format!("patchbay-browser-{stamp}-{}", std::process::id()));
        std::fs::create_dir(&output)?;
        let status = Command::new("node")
            .current_dir(&root)
            .args([
                "proof/browser/node_modules/@playwright/test/cli.js",
                "test",
                "--config",
                "proof/browser/pete-cognition.playwright.config.mjs",
                "--workers=1",
                "--retries=0",
            ])
            .arg("--output")
            .arg(output.join("runner"))
            .env("CONDUIT_PETE_INSPECT_BIN", binary)
            .env("CONDUIT_PETE_COGNITION_EVIDENCE", &evidence)
            .env("CONDUIT_PETE_BROWSER_OUTPUT", &output)
            .status()?;
        if !status.success() {
            return Err(format!(
                "historical cognition browser proof failed; retained {}",
                output.display()
            )
            .into());
        }
        if !opts.quiet {
            println!("historical Patchbay browser evidence: {}", output.display());
        }
    }
    Ok(())
}
