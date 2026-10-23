//! Native wait possession and its zero-result/configuration prerequisites.
use super::execute;
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    for (package, target) in [
        ("conduit-core", "base_capability"),
        ("conduit-plan-lowering", "selected_subject"),
    ] {
        execute(
            Command::new("cargo")
                .current_dir(root)
                .args(["test", "--locked", "-p", package, "--test", target]),
        )?;
    }
    execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "test",
                "--locked",
                "--manifest-path",
                "mechanisms/io/monotonic-wait/Cargo.toml",
            ])
            .env("CARGO_TARGET_DIR", root.join("target/environment-runtime")),
    )
}
