//! Actual catalogue checkpoint file/process/kernel CPU qualification.
use std::{error::Error, io::Write, path::Path, process::Command};

pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    if !cfg!(target_os = "linux") {
        return Err("catalogue file/process qualification requires Linux".into());
    }
    let output = Command::new("cargo")
        .current_dir(root)
        .args([
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--test",
            "environment_catalogue_checkpoint",
        ])
        .output()?;
    std::io::stdout().write_all(&output.stdout)?;
    std::io::stderr().write_all(&output.stderr)?;
    if !output.status.success() {
        return Err(format!("catalogue runtime proof failed: {}", output.status).into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout
        .lines()
        .any(|line| line.starts_with("test result: ok. 4 passed; 0 failed; 0 ignored;"))
    {
        return Err("catalogue runtime proof did not execute its four required checks".into());
    }
    let status = Command::new("cargo")
        .current_dir(root)
        .args([
            "clippy",
            "--locked",
            "-p",
            "conduit-std-host",
            "--test",
            "environment_catalogue_checkpoint",
            "--",
            "-D",
            "warnings",
        ])
        .status()?;
    if !status.success() {
        return Err(format!("catalogue runtime lint failed: {status}").into());
    }
    Ok(())
}
