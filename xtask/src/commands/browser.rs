//! Repository lifecycle entrance for one independent browser Host.

use crate::cli::GlobalOpts;
use crate::process::{run_step, Step};
use crate::workspace::workspace_root;

pub fn run(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    run_step(
        &Step::new(
            "browser.runtime",
            "Build the bounded browser Host runtime",
            "cargo",
            &[
                "build",
                "-p",
                "conduit-browser-runtime",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
            ],
        ),
        &root,
        opts,
    )?;
    run_step(
        &Step::new(
            "browser.host",
            "Launch one independent browser page/WASM Host",
            "cargo",
            &["run", "-p", "conduit-browser-host"],
        ),
        &root,
        opts,
    )?;
    Ok(())
}
