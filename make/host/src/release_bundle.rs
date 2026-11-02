//! Exact byte inventory of an existing reviewed Host release. This preserves
//! the established host-bundle@1 aggregate; it does not equate a raw file SHA
//! with a bundle identity, or prove a target's richer IMAGE/Boot semantics.
use crate::ReleaseTargetProfile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;

pub const HOST_RELEASE_BUNDLE_SCHEMA: &str = "conduit.release/host-bundle@1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostReleaseFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub media_type: String,
}

/// The existing release format, including target-owned extension fields. Those
/// fields are retained, not interpreted as generic target-readiness evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostReleaseManifest {
    pub schema: String,
    pub target_id: String,
    pub make_package_id: String,
    pub output: String,
    pub builder_adapter: String,
    pub deployment_adapter: String,
    pub source_identity: String,
    pub bundle_sha256: String,
    pub files: Vec<HostReleaseFile>,
    #[serde(flatten)]
    pub target_metadata: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostReleaseBounds {
    pub maximum_manifest_bytes: usize,
    pub maximum_files: usize,
    pub maximum_file_bytes: u64,
    pub maximum_bundle_bytes: u64,
}

/// Opaque proof that these manifest bytes and every declared payload were read
/// and verified together. No public digest-only constructor exists. The caller
/// still owns prior authorization, immutable custody and later target checking.
pub struct VerifiedHostReleaseBytes {
    manifest: HostReleaseManifest,
    manifest_bytes: Vec<u8>,
    payload_bytes: u64,
    prefixes: std::collections::BTreeMap<String, Vec<u8>>,
}
impl VerifiedHostReleaseBytes {
    pub fn manifest(&self) -> &HostReleaseManifest {
        &self.manifest
    }
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }
    pub fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }
    pub fn image_content_digest(&self) -> &str {
        &self.manifest.bundle_sha256
    }
    /// At most eight actually observed prefix bytes; target checkers can require
    /// format magic without rereading an unretained payload or trusting a name.
    pub fn payload_prefix(&self, path: &str) -> Option<&[u8]> {
        self.prefixes.get(path).map(Vec::as_slice)
    }
}

/// Verify an explicitly selected existing release. `open` is the owning Host's
/// admitted source-read operation, not ambient filesystem authority. Readers
/// start at byte zero. No undeclared pathname is requested. Read work consumes
/// at most each declared length plus one probe byte. Cancellation is checked
/// before opening and between reads. Success is an observation, not a lock on
/// files: the packaging owner must retain/reverify the actual bytes it uses.
pub fn verify_host_release_bytes<R: Read>(
    bytes: &[u8],
    expected: &ReleaseTargetProfile<'_>,
    expected_source: &str,
    bounds: HostReleaseBounds,
    mut open: impl FnMut(&str) -> Result<R, String>,
    mut cancelled: impl FnMut() -> bool,
) -> Result<VerifiedHostReleaseBytes, String> {
    if bytes.is_empty()
        || bytes.len() > bounds.maximum_manifest_bytes
        || !(1..=256 * 1024).contains(&bounds.maximum_manifest_bytes)
        || !(1..=32).contains(&bounds.maximum_files)
        || bounds.maximum_file_bytes == 0
        || bounds.maximum_bundle_bytes == 0
        || expected_source.is_empty()
        || expected_source.len() > 256
    {
        return Err("Host release exceeds its admitted manifest/bundle bounds".into());
    }
    let manifest: HostReleaseManifest =
        serde_json::from_slice(bytes).map_err(|e| format!("Host release manifest: {e}"))?;
    if manifest.schema != HOST_RELEASE_BUNDLE_SCHEMA
        || manifest.target_id != expected.target_id
        || manifest.make_package_id != expected.package_id
        || manifest.output != expected.output
        || manifest.builder_adapter != expected.builder_adapter
        || manifest.deployment_adapter != expected.deployment_adapter
        || manifest.source_identity != expected_source
        || manifest.files.is_empty()
        || manifest.files.len() > bounds.maximum_files
        || !digest(&manifest.bundle_sha256)
    {
        return Err("Host release differs from its exact selected Make/source identity".into());
    }
    let mut total = 0u64;
    let mut aggregate = Sha256::new();
    aggregate.update(b"conduit.release/host-bundle-content@1\0");
    for (index, file) in manifest.files.iter().enumerate() {
        if !safe_path(&file.path)
            || file.bytes == 0
            || file.bytes > bounds.maximum_file_bytes
            || !digest(&file.sha256)
            || file.media_type.is_empty()
            || file.media_type.len() > 128
            || manifest.files[..index]
                .iter()
                .any(|prior| prior.path == file.path)
        {
            return Err("Host release has an unsafe, duplicate or unbounded payload".into());
        }
        total = total
            .checked_add(file.bytes)
            .filter(|n| *n <= bounds.maximum_bundle_bytes)
            .ok_or("Host release payload bound exhausted")?;
        aggregate.update(file.path.as_bytes());
        aggregate.update(b"\0");
        aggregate.update(file.sha256.as_bytes());
        aggregate.update(b"\n");
    }
    if format!("sha256:{:x}", aggregate.finalize()) != manifest.bundle_sha256 {
        return Err("Host release file-inventory aggregate identity mismatch".into());
    }
    let mut buffer = [0u8; 8192];
    let mut prefixes = std::collections::BTreeMap::new();
    for file in &manifest.files {
        if cancelled() {
            return Err("Host release verification cancelled".into());
        }
        let mut reader = open(&file.path)?;
        let mut count = 0u64;
        let mut hash = Sha256::new();
        let mut prefix = Vec::with_capacity(8);
        loop {
            if cancelled() {
                return Err("Host release verification cancelled".into());
            }
            let capacity = (file.bytes - count).min(buffer.len() as u64).max(1) as usize;
            let n = reader
                .read(&mut buffer[..capacity])
                .map_err(|e| format!("Host release payload read: {e}"))?;
            if n == 0 {
                break;
            }
            if n as u64 > file.bytes - count {
                return Err("Host release payload exceeds reviewed length".into());
            }
            count += n as u64;
            hash.update(&buffer[..n]);
            let prefix_count = (8 - prefix.len()).min(n);
            prefix.extend_from_slice(&buffer[..prefix_count]);
        }
        prefixes.insert(file.path.clone(), prefix);
        if count != file.bytes || format!("sha256:{:x}", hash.finalize()) != file.sha256 {
            return Err("Host release payload differs from reviewed bytes".into());
        }
    }
    if cancelled() {
        return Err("Host release verification cancelled".into());
    }
    Ok(VerifiedHostReleaseBytes {
        manifest,
        manifest_bytes: bytes.to_vec(),
        payload_bytes: total,
        prefixes,
    })
}
fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn safe_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.starts_with('/')
        && !value
            .bytes()
            .any(|b| b <= b' ' || b == 127 || b"\\%:?#".contains(&b))
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

#[cfg(test)]
mod tests;
