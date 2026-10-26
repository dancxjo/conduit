//! Build-side Crèche selection and durable identity; no flash or Boot claim.
use std::{error::Error, path::Path, process::Command};

pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    for (manifest, tests) in [
        (
            "architecture/core/Cargo.toml",
            &[
                "configuration_text",
                "optional_plan_storage",
                "fragment_payload_hash",
                "incremental_fragment_hash",
            ][..],
        ),
        (
            "bodies/environment/make/Cargo.toml",
            &[
                "body_image",
                "resident_source",
                "linked_material",
                "indexed_planning",
                "display_indexed_planning",
                "display_local_kernel",
            ][..],
        ),
    ] {
        for operation in ["test", "clippy"] {
            let mut command = Command::new("cargo");
            command
                .current_dir(root)
                .args([operation, "--locked", "--manifest-path", manifest]);
            for test in tests {
                command.args(["--test", test]);
            }
            if operation == "clippy" {
                command.args(["--", "-D", "warnings"]);
            }
            command.env(
                "CARGO_TARGET_DIR",
                std::env::var_os("CARGO_TARGET_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| root.join("target")),
            );
            super::execute(&mut command)?;
        }
    }
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command.current_dir(root).args([
            operation,
            "--locked",
            "-p",
            "conduit-planner",
            "--features",
            "indexed-source",
            "--lib",
        ]);
        if operation == "clippy" {
            command.args(["--", "-D", "warnings"]);
        }
        command.env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| root.join("target")),
        );
        super::execute(&mut command)?;
    }
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command.current_dir(root).args([
            operation,
            "--locked",
            "--manifest-path",
            "bodies/environment/runtime/Cargo.toml",
            "--features",
            "construction",
            "--lib",
        ]);
        if operation == "test" {
            command.args(["--test", "capture_kernel", "--test", "local_fore"]);
        }
        if operation == "clippy" {
            command.args(["--", "-D", "warnings"]);
        }
        command.env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| root.join("target")),
        );
        super::execute(&mut command)?;
    }
    Ok(())
}
