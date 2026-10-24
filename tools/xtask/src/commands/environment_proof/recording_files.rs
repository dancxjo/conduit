//! Planned production recovery kernel with actual local Linux file effects.
use super::{execute, host_target};
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    if !cfg!(target_os = "linux") {
        return Err("recording-files requires Linux".into());
    }
    execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "--manifest-path",
        "bodies/environment/make/Cargo.toml",
        "--test",
        "recording_recovery",
        "linux_files",
    ]))?;
    execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "--manifest-path",
        "bodies/environment/make/Cargo.toml",
        "--test",
        "recording_recovery",
        "--",
        "-D",
        "warnings",
    ]))
}
