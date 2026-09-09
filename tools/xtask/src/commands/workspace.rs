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
                    "workspace-library.spec.mjs",
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
