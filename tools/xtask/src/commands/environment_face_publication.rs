//! Retained producer Face state and its finite Mask; no hardware or membership claim.
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
            "face-projection,display-mask",
            "--test",
            "canonical_input",
            "--test",
            "publication",
            "--test",
            "retained_publication_face",
            "--test",
            "publication_validation",
            "--test",
            "publication_face_custody",
        ]);
        if operation == "clippy" {
            command.args(["--", "-D", "warnings"]);
        }
        target_directory(root, &mut command);
        super::execute(&mut command)?;

        // This Mask is an independent workspace: root workspace checks and
        // enabling its runtime dependency do not execute its own conformance.
        let mut mask = Command::new("cargo");
        mask.current_dir(root).args([
            operation,
            "--locked",
            "--manifest-path",
            "plots/seven-segment-mask/Cargo.toml",
        ]);
        if operation == "clippy" {
            mask.args(["--all-targets", "--", "-D", "warnings"]);
        }
        target_directory(root, &mut mask);
        super::execute(&mut mask)?;
    }
    let mut docs = Command::new("cargo");
    docs.current_dir(root).args([
        "test",
        "--locked",
        "--manifest-path",
        "bodies/environment/runtime/Cargo.toml",
        "--features",
        "face-projection,display-mask",
        "--doc",
        "publication_face_custody",
    ]);
    target_directory(root, &mut docs);
    super::execute(&mut docs)?;
    Ok(())
}
fn target_directory(root: &Path, command: &mut Command) {
    command.env(
        "CARGO_TARGET_DIR",
        std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| root.join("target")),
    );
}
