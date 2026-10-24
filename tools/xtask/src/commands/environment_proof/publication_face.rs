//! Shared typed publication-to-Face projection, without a transport fixture.
use super::execute;
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "--manifest-path",
        "bodies/environment/face/Cargo.toml",
    ]))?;
    execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "--manifest-path",
        "bodies/environment/runtime/Cargo.toml",
        "--features",
        "face-projection",
        "--test",
        "publication_face",
    ]))
}
