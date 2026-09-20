//! Repository entrance for retained Body arrival through the browser Host.
use crate::cli::GlobalOpts;
use crate::process::{run_step, Step};
use crate::workspace::workspace_root;
use clap::Args;

#[derive(Args, Debug)]
pub struct WorkspaceArgs {
    /// Run the pinned browser arrival proof instead of opening a window.
    #[arg(long)]
    check: bool,
}

pub fn run(args: &WorkspaceArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    if args.check {
        run_step(
            &Step::new(
                "demo.workspace.rendezvous-host",
                "Build the installed Host entrance used by running-Host proof",
                "cargo",
                &["build", "-p", "conduit"],
            ),
            &root,
            opts,
        )?;
    }
    let releases = root.join("target/workspace-release-artifacts");
    if releases.exists() && !opts.dry_run {
        std::fs::remove_dir_all(&releases)?;
    }
    run_step(
        &Step::new(
            "demo.workspace.browser-release",
            "Seal the reviewed compiler-free browser Host distribution",
            "cargo",
            &[
                "xtask",
                "host",
                "release",
                "--platform",
                "browser",
                "--output",
                "target/workspace-release-artifacts",
            ],
        ),
        &root,
        opts,
    )?;
    run_step(
        &Step::new(
            "demo.workspace.release-catalog",
            "Seal the Workspace browser Host release catalog",
            "cargo",
            &[
                "xtask",
                "host",
                "release-catalog",
                "--root",
                "target/workspace-release-artifacts",
                "--generation",
                "1",
            ],
        ),
        &root,
        opts,
    )?;
    // Host release fabrication also builds a narrower browser runtime in the
    // shared target directory. Build the Workspace-featured runtime last so
    // the staging input cannot be replaced by that intermediate artifact.
    run_step(
        &Step::new(
            "demo.workspace.runtime",
            "Build the shared Crèche and ordinary Body execution runtime",
            "cargo",
            &[
                "build",
                "-p",
                "conduit-browser-runtime",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "--no-default-features",
                "--features",
                "creche-surface,form-runner",
            ],
        ),
        &root,
        opts,
    )?;
    let product = root.join("target/workspace-product");
    if product.exists() && !opts.dry_run {
        std::fs::remove_dir_all(product)?;
    }
    run_step(
        &Step::new(
            "demo.workspace.package",
            "Stage the exact Body arrival application",
            "sh",
            &[
                "products/workspace/tools/stage-workspace-product.sh",
                "target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
                "target/workspace-product",
                "target/workspace-release-artifacts",
            ],
        ),
        &root,
        opts,
    )?;
    if args.check {
        run_step(
            &Step::new(
                "demo.workspace.input",
                "Check foreground routing and Host retirement",
                "node",
                &[
                    "--test",
                    "proof/browser/browser-body-input.test.mjs",
                    "proof/browser/browser-body-host.test.mjs",
                    "proof/browser/workspace-handoff.test.mjs",
                    "proof/browser/browser-form-effects.test.mjs",
                ],
            ),
            &root,
            opts,
        )?;
        run_step(
            &Step::new(
                "demo.workspace.arrival",
                "Prove Birth, listening Forms, continuity, and storage refusal",
                "node",
                &[
                    "proof/browser/node_modules/@playwright/test/cli.js",
                    "test",
                    "--config",
                    "proof/browser/playwright.config.mjs",
                    "--project",
                    "chromium",
                    "--workers",
                    "1",
                    "--retries",
                    "0",
                    "workspace-arrival.spec.mjs",
                    "workspace-birth-naming.spec.mjs",
                    "workspace-body-execution.spec.mjs",
                    "workspace-membership.spec.mjs",
                    "workspace-library.spec.mjs",
                    "workspace-resident-applications.spec.mjs",
                ],
            ),
            &root,
            opts,
        )?;
    } else {
        run_step(
            &Step::new(
                "demo.workspace.host",
                "Open the Body arrival experience",
                "cargo",
                &[
                    "run",
                    "-p",
                    "conduit-browser-host",
                    "--",
                    "--application",
                    "target/workspace-product",
                    "--mount",
                    "/workspace/",
                ],
            ),
            &root,
            opts,
        )?;
    }
    Ok(())
}
