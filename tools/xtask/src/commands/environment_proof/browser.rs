//! Actual IndexedDB persistence and original recorder-kernel read completion.
use super::execute;
use std::{error::Error, path::Path, process::Command};

pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    for (target, filter) in [
        ("recording_browser", "overnight_catalog"),
        ("recording_recovery", "original_kernel_reads"),
    ] {
        execute(
            Command::new("cargo")
                .current_dir(root)
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    "bodies/environment/make/Cargo.toml",
                    "--features",
                    "browser-storage-proof",
                    "--test",
                    target,
                    filter,
                    "--",
                    "--ignored",
                    "--nocapture",
                ])
                .env("CARGO_TARGET_DIR", root.join("target")),
        )?;
    }
    Ok(())
}
