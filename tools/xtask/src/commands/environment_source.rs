//! Source-to-quantity conversion, without hardware or installed Body claims.
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    for capture in [false, true] {
        for operation in ["test", "clippy"] {
            let mut command = Command::new("cargo");
            command.current_dir(root).args([
                operation,
                "--locked",
                "--manifest-path",
                "bodies/environment/runtime/Cargo.toml",
                "--test",
                "source_frames",
            ]);
            if capture {
                command.args(["--features", "source-capture"]);
            }
            if operation == "clippy" {
                command.args(["--", "-D", "warnings"]);
            }
            if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
                command.env("CARGO_TARGET_DIR", target);
            } else {
                command.env("CARGO_TARGET_DIR", root.join("target"));
            }
            if !command.status()?.success() {
                return Err("environment Source proof failed".into());
            }
        }
    }
    Ok(())
}
