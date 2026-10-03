//! Authenticate and bound the installed owner Face snapshot and exact executable.
use conduit_core::HostAdvertisement;
use conduit_presentation::Presentation;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path, process::Command};

const SNAPSHOT_SCHEMA: &str = "conduit.body/local-face-snapshot@1";
const MAX_FACE_BYTES: usize = 1024 * 1024;

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnerSnapshot {
    pub(super) schema: String,
    pub(super) presentation: Presentation,
    pub(super) advertisement: HostAdvertisement,
}

pub(super) fn face_bytes(bin: &Path, state_dir: &Path) -> Result<Vec<u8>, String> {
    let result = Command::new(bin)
        .args(["body", "face", "--state-dir"])
        .arg(state_dir)
        .arg("--json")
        .output()
        .map_err(|error| error.to_string())?;
    if !result.status.success() {
        return Err(format!(
            "installed owner Face snapshot failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    if result.stdout.len() > MAX_FACE_BYTES {
        return Err("owner Face snapshot exceeded byte bound".into());
    }
    Ok(result.stdout)
}

pub(super) fn parse_snapshot(bytes: &[u8]) -> Result<OwnerSnapshot, String> {
    let snapshot: OwnerSnapshot =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if snapshot.schema != SNAPSHOT_SCHEMA {
        return Err("unexpected owner Face snapshot schema".into());
    }
    snapshot
        .presentation
        .validate()
        .map_err(|error| format!("invalid owner Face: {error:?}"))?;
    if snapshot.presentation.basis.body_id.is_none() {
        return Err("owner Face did not name a Body".into());
    }
    Ok(snapshot)
}

pub(super) fn hash_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = fs::File::open(path)?;
    let mut sha = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        sha.update(&buffer[..n]);
    }
    Ok(format!("{:x}", sha.finalize()))
}

pub(super) fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let result = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    if !result.status.success() {
        return Err(format!("git {} failed", args.join(" ")));
    }
    String::from_utf8(result.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| error.to_string())
}
