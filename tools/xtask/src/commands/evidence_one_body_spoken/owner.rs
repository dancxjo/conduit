//! Authenticate and bound the installed owner Face snapshot and exact executable.
use conduit_core::HostAdvertisement;
use conduit_presentation::Presentation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::Path,
    process::{Command, Stdio},
};

const SNAPSHOT_SCHEMA: &str = "conduit.body/local-face-snapshot@1";
const MAX_FACE_BYTES: usize = 1024 * 1024;
const MAX_INSTALLATION_BYTES: usize = 64 * 1024;

#[derive(Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct InstalledRelease {
    pub(super) schema: String,
    pub(super) host_id: String,
    pub(super) release_source_identity: String,
    pub(super) release_bundle_sha256: String,
    pub(super) product_executable: String,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnerSnapshot {
    pub(super) schema: String,
    pub(super) presentation: Presentation,
    pub(super) advertisement: HostAdvertisement,
}

pub(super) fn face_bytes(bin: &Path, state_dir: &Path) -> Result<Vec<u8>, String> {
    let mut owner = Command::new(bin)
        .args(["body", "face", "--state-dir"])
        .arg(state_dir)
        .arg("--json")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    owner
        .stdout
        .take()
        .ok_or("owner stdout was not piped")?
        .take((MAX_FACE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_FACE_BYTES {
        let _ = owner.kill();
        let _ = owner.wait();
        return Err("owner Face snapshot exceeded byte bound".into());
    }
    if !owner.wait().map_err(|error| error.to_string())?.success() {
        return Err("installed owner Face snapshot failed".into());
    }
    Ok(bytes)
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

pub(super) fn installed_release(
    state_dir: &Path,
    bin: &Path,
    source_commit: &str,
    owner_host_id: &str,
) -> Result<InstalledRelease, String> {
    let path = state_dir.join("installation.json");
    if fs::symlink_metadata(&path)
        .map_err(|error| error.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("installed owner release record is a symlink".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .map_err(|error| error.to_string())?
        .take((MAX_INSTALLATION_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_INSTALLATION_BYTES {
        return Err("installed owner release record exceeded byte bound".into());
    }
    let release: InstalledRelease =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if release.schema != "conduit.install/durable-host@1"
        || release.release_source_identity != source_commit
        || release.host_id != owner_host_id
        || release.release_bundle_sha256.len() != 64
        || !release
            .release_bundle_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || fs::canonicalize(&release.product_executable).map_err(|error| error.to_string())? != bin
    {
        return Err(
            "installed owner release does not match exact source, Host, or executable".into(),
        );
    }
    Ok(release)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn installed_release_refuses_foreign_source_and_host() {
        let state = std::env::temp_dir().join(format!(
            "conduit-journey-installed-release-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&state).unwrap();
        let bin = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        let write = |source: &str, host: &str| {
            fs::write(
                state.join("installation.json"),
                serde_json::to_vec(&json!({
                    "schema": "conduit.install/durable-host@1",
                    "host_id": host,
                    "release_source_identity": source,
                    "release_bundle_sha256": "0".repeat(64),
                    "product_executable": bin,
                    "body_state": {"not": "included in retained proof"},
                }))
                .unwrap(),
            )
            .unwrap();
        };
        write("source-a", "host-a");
        let retained = installed_release(&state, &bin, "source-a", "host-a").unwrap();
        assert!(!serde_json::to_string(&retained)
            .unwrap()
            .contains("body_state"));
        assert!(installed_release(&state, &bin, "source-b", "host-a").is_err());
        assert!(installed_release(&state, &bin, "source-a", "host-b").is_err());
        write("source-b", "host-a");
        assert!(installed_release(&state, &bin, "source-a", "host-a").is_err());
        fs::remove_dir_all(state).unwrap();
    }
}
