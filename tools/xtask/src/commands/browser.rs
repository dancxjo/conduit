//! Repository lifecycle entrance for one independent browser Host.

use crate::cli::GlobalOpts;
use crate::process::{run_step, Step};
use crate::workspace::workspace_root;

// Shared by the interactive entrance and proof suite, so both consume the
// canonical optimized release copy rather than Cargo's diagnostic artifact.
pub(crate) const RUNTIME_RELEASE_ARGS: &[&str] = &[
    "xtask",
    "make",
    "host",
    "release",
    "--platform",
    "browser",
    "--output",
    "target/browser-host-release",
];
pub(crate) const RUNTIME_ENVIRONMENT: &[(&str, &str)] = &[(
    "CONDUIT_BROWSER_RUNTIME_WASM",
    "target/browser-host-release/runtime.wasm",
)];

pub fn run(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    run_step(
        &Step::new(
            "browser.runtime",
            "Seal the optimized browser Host runtime",
            "cargo",
            RUNTIME_RELEASE_ARGS,
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
        )
        .with_environment(RUNTIME_ENVIRONMENT),
        &root,
        opts,
    )?;
    Ok(())
}
