//! Shared verification primitives for AVR acquisition and firmware builds.
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Output};

pub(super) fn require_success(
    output: &Output,
    action: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{action} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
    .into())
}

pub(super) fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
