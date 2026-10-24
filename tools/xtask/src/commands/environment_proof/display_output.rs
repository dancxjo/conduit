//! Finite exact display output possession and existing Mask/packet regression.
use super::{execute, host_target};
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                operation,
                "--locked",
                "--manifest-path",
                "mechanisms/io/tm1637/Cargo.toml",
                "--features",
                "host-call",
            ]);
        if operation == "clippy" {
            command.args(["--all-targets", "--", "-D", "warnings"]);
        }
        execute(&mut command)?;
    }
    for (manifest, test) in [
        ("bodies/environment/runtime/Cargo.toml", "display_packets"),
        ("bodies/environment/make/Cargo.toml", "native_face_mask"),
        ("bodies/environment/make/Cargo.toml", "display_output"),
    ] {
        for operation in ["test", "clippy"] {
            let mut command = Command::new("cargo");
            command
                .current_dir(root)
                .env("CARGO_TARGET_DIR", host_target(root))
                .args([
                    operation,
                    "--locked",
                    "--manifest-path",
                    manifest,
                    "--test",
                    test,
                ]);
            if operation == "clippy" {
                command.args(["--", "-D", "warnings"]);
            }
            execute(&mut command)?;
        }
    }
    Ok(())
}
