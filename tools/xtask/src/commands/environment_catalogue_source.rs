//! Authored catalogue checkpoint and exact shared read-only seed; Source proof only.
use std::{error::Error, io::Write, path::Path, process::Command};

pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    let manifest = "bodies/environment/make/Cargo.toml";
    let output = Command::new("cargo")
        .current_dir(root)
        .args([
            "test",
            "--locked",
            "--manifest-path",
            manifest,
            "--test",
            "catalogue_checkpoint",
        ])
        .output()?;
    std::io::stdout().write_all(&output.stdout)?;
    std::io::stderr().write_all(&output.stderr)?;
    if !output.status.success() {
        return Err(format!("catalogue Source proof failed: {}", output.status).into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout
        .lines()
        .any(|line| line.starts_with("test result: ok. 7 passed; 0 failed; 0 ignored;"))
    {
        return Err("catalogue Source proof did not execute its seven required checks".into());
    }
    let status = Command::new("cargo")
        .current_dir(root)
        .args([
            "clippy",
            "--locked",
            "--manifest-path",
            manifest,
            "--test",
            "catalogue_checkpoint",
            "--",
            "-D",
            "warnings",
        ])
        .status()?;
    if !status.success() {
        return Err(format!("catalogue Source lint failed: {status}").into());
    }
    Ok(())
}
