//! Authored finite sampling and retained software owners; no physical Body claim.
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command
            .current_dir(root)
            .env("CARGO_TARGET_DIR", &target)
            .args([
                operation,
                "--locked",
                "--manifest-path",
                "bodies/environment/runtime/Cargo.toml",
                "--features",
                "construction",
                "--lib",
                "--test",
                "construction",
                "--test",
                "sampling_value_extents",
                "--test",
                "appliance_kernel",
            ]);
        if operation == "clippy" {
            command.args(["--", "-D", "warnings"]);
        }
        super::execute(&mut command)?;
    }
    super::execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", target)
            .args([
                "check",
                "--locked",
                "--manifest-path",
                "bodies/environment/runtime/Cargo.toml",
                "--no-default-features",
                "--features",
                "appliance",
                "--lib",
            ]),
    )
}
