//! Capture existing typed compiler arguments across the Make subprocess boundary.
//! Source remains authoritative; the target compiler checks exact selections.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
const MAXIMUM_BYTES: u64 = 64 * 1024;
pub(super) struct Captured {
    path: PathBuf,
    pub sha256: String,
}
impl Captured {
    pub fn capture(
        input: &Path,
        source_sha256: &str,
        output: &Path,
        dry_run: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = read(input, source_sha256)?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let path = if dry_run {
            input.canonicalize()?
        } else {
            fs::create_dir_all(output)?;
            let path = output.join("workset-specializations.json");
            fs::write(&path, bytes)?;
            path.canonicalize()?
        };
        Ok(Self { path, sha256 })
    }
    pub fn configure(&self, command: &mut Command) {
        command
            .env("CONDUIT_WORKSET_SPECIALIZATIONS", &self.path)
            .env("CONDUIT_WORKSET_SPECIALIZATIONS_SHA256", &self.sha256);
    }
    pub fn verify(&self, source_sha256: &str) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = read(&self.path, source_sha256)?;
        if format!("{:x}", Sha256::digest(bytes)) != self.sha256 {
            return Err("captured typed compiler selections changed during build".into());
        }
        Ok(())
    }
}
fn read(path: &Path, source_sha256: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAXIMUM_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() as u64 > MAXIMUM_BYTES {
        return Err("typed compiler selections extent refused".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    if value.get("schema").and_then(|v| v.as_u64()) != Some(1)
        || value.get("source_sha256").and_then(|v| v.as_str()) != Some(source_sha256)
    {
        return Err("typed compiler selections differ from original Source".into());
    }
    // The shared target compiler owns the strict complete receipt grammar,
    // finite bounds, Type resolution, canonical Kind identity and Source check.
    Ok(bytes)
}
