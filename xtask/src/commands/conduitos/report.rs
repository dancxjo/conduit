use std::{fs::File, io::Read, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ConduitosError;

#[derive(Debug, Serialize)]
pub struct BuildRecord {
    pub schema: &'static str,
    pub base_commit: String,
    pub architecture: &'static str,
    pub rust_target: &'static str,
    pub limine_crate: &'static str,
    pub elf_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct ImageRecord {
    pub schema: &'static str,
    pub architecture: &'static str,
    pub limine_version: &'static str,
    pub limine_archive_sha256: &'static str,
    pub iso_sha256: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestBootSign {
    pub schema: String,
    pub status: String,
    pub arch: String,
    pub firmware: String,
    pub build_id: String,
    pub image_id: String,
    pub limine: String,
    pub qemu_profile: String,
    pub host_id: String,
    pub boot_id: String,
    pub memory_regions: u16,
    pub artifacts: u16,
    pub framebuffers: u8,
    pub command_line_bytes: u16,
    pub runtime_arena_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ProofRecord {
    pub schema: &'static str,
    pub base_commit: String,
    pub architecture: &'static str,
    pub proof_class: &'static str,
    pub limine_version: &'static str,
    pub limine_archive_sha256: &'static str,
    pub qemu_profile: &'static str,
    pub qemu_version: String,
    pub iso_sha256: String,
    pub reproducible_image: bool,
    pub first_boot: GuestBootSign,
    pub second_boot: GuestBootSign,
    pub fresh_host_id: bool,
    pub fresh_boot_id: bool,
}

pub fn sha256_file(path: &Path) -> Result<String, ConduitosError> {
    let mut file = File::open(path).map_err(|error| {
        ConduitosError::refusal(
            "artifact-unavailable",
            format!("cannot open {}: {error}", path.display()),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            ConduitosError::refusal(
                "artifact-unavailable",
                format!("cannot read {}: {error}", path.display()),
            )
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn git_head(root: &Path) -> Result<String, ConduitosError> {
    let output = super::profile::command(
        "git",
        &["rev-parse", "HEAD"],
        root,
        "base-commit-unavailable",
    )?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
