//! Retained producer Face state; no hardware, membership or recording claim.
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command.current_dir(root).args([
            operation,
            "--locked",
            "--manifest-path",
            "bodies/environment/runtime/Cargo.toml",
            "--features",
            "face-projection",
            "--test",
            "retained_publication_face",
            "--test",
            "publication_validation",
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
    Ok(())
}
