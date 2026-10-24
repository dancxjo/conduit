//! Checked output construction; this entrance installs no Body or native Play.
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    for (manifest, test) in [
        ("bodies/environment/make/Cargo.toml", "display_output"),
        ("bodies/environment/runtime/Cargo.toml", "display_packets"),
        (
            "bodies/environment/runtime/Cargo.toml",
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
                "--test",
                test,
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
    }
    Ok(())
}
