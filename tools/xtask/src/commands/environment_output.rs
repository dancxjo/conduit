//! Checked output construction; this entrance installs no Body or native Play.
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    for (manifest, selector, test) in [
        ("bodies/environment/make/Cargo.toml", "--test", "mask_plan"),
        (
            "architecture/plot/Cargo.toml",
            "--test",
            "compact_scalar_children",
        ),
        (
            "bodies/environment/make/Cargo.toml",
            "--test",
            "display_output",
        ),
        (
            "bodies/environment/make/Cargo.toml",
            "--test",
            "native_root_policy",
        ),
        (
            "bodies/environment/make/Cargo.toml",
            "--test",
            "display_compact",
        ),
        (
            "bodies/environment/make/Cargo.toml",
            "--test",
            "display_session",
        ),
        (
            "bodies/environment/host/Cargo.toml",
            "--test",
            "display_evidence",
        ),
        (
            "bodies/environment/host/Cargo.toml",
            "--bin",
            "conduit-environment-host",
        ),
        (
            "bodies/environment/make/Cargo.toml",
            "--test",
            "display_compact_kernel",
        ),
        (
            "bodies/environment/runtime/Cargo.toml",
            "--test",
            "display_packets",
        ),
        (
            "bodies/environment/runtime/Cargo.toml",
            "--test",
            "observation_templates",
        ),
        (
            "bodies/environment/runtime/Cargo.toml",
            "--test",
            "display_value_budget",
        ),
        (
            "bodies/environment/runtime/Cargo.toml",
            "--test",
            "display_frame_material",
        ),
    ] {
        for operation in ["test", "clippy"] {
            let mut command = Command::new("cargo");
            command.current_dir(root).args([
                operation,
                "--locked",
                "--manifest-path",
                manifest,
                selector,
                test,
            ]);
            if manifest == "bodies/environment/runtime/Cargo.toml" {
                command.args(["--features", "display-output"]);
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
    Ok(())
}
