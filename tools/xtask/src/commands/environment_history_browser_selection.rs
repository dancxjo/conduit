//! Independent complete browser Look intent through the existing owner seam.
use super::super::execute;
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    execute(Command::new("node").current_dir(root).args([
        "--test",
        "targets/browser/host/assets/browser-runtime-bridge.test.mjs",
        "targets/browser/host/assets/browser-membership-history.test.mjs",
        "targets/browser/host/assets/browser-membership-wardrobe.test.mjs",
        "targets/browser/sdk/browser-sdk-owner-show-state.test.mjs",
        "targets/browser/sdk/browser-sdk-history.test.mjs",
    ]))?;
    for (package, filter) in [
        ("conduit-browser-runtime", "history_look_intent::tests"),
        ("conduit-std-host", "browser_admission::history_selection"),
    ] {
        execute(
            Command::new("cargo")
                .current_dir(root)
                .args(["test", "--locked", "-p", package, "--lib", filter]),
        )?;
    }
    execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit",
        "--bin",
        "conduit",
        "browser_mask_planning_requires_the_reviewed_back_and_face_resource",
    ]))
}
pub(super) fn lint(root: &Path) -> Result<(), Box<dyn Error>> {
    execute(Command::new("cargo").current_dir(root).args([
        "clippy", "--locked", "-p", "conduit", "--bin", "conduit", "--", "-D", "warnings",
    ]))?;
    execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-browser-runtime",
        "-p",
        "conduit-std-host",
        "--lib",
        "--",
        "-D",
        "warnings",
    ]))
}
pub(super) fn browser(root: &Path) -> Result<(), Box<dyn Error>> {
    execute(Command::new("node").current_dir(root).args([
        "proof/browser/node_modules/@playwright/test/cli.js",
        "test",
        "--config",
        "proof/browser/playwright.config.mjs",
        "owner-look-controls.spec.mjs",
        "--project",
        "chromium",
        "--workers=1",
        "--retries=0",
    ]))
}
