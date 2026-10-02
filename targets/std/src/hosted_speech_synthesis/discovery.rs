use super::EspeakFailure;
use conduit_core::{
    kind_id, ResourceAccessMode, ResourceContentRequirement, ResourceRetention,
    ResourceSemanticIdentity, ResourceSharing, ResourceVersionIdentity,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
const MAXIMUM_FILES: usize = 4096;
const MAXIMUM_BYTES: u64 = 128 * 1024 * 1024;

/// Trusted installed-engine selection, not hostile-code confinement. The engine
/// library and whole voice data tree are content-bound; system loader/OS remain
/// part of the explicitly trusted installed Host platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspeakDiscovery {
    pub(super) executable: PathBuf,
    pub(super) data_root: PathBuf,
    pub(super) engine: PathBuf,
    pub(super) voice: String,
    dependencies: Vec<PathBuf>,
    pub provider_sha256: String,
    digest: [u8; 32],
    bytes: u32,
    items: u32,
}
impl EspeakDiscovery {
    pub fn inspect(
        executable: &Path,
        data_root: &Path,
        voice: &str,
        dependencies: &[PathBuf],
    ) -> Result<Self, EspeakFailure> {
        if voice.is_empty()
            || voice.len() > 64
            || !voice
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || dependencies.is_empty()
            || dependencies.len() > 16
        {
            return Err(EspeakFailure::InvalidProvider);
        }
        let executable = regular(executable)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(&executable)
                .map_err(|_| EspeakFailure::InvalidProvider)?
                .permissions()
                .mode()
                & 0o111
                == 0
            {
                return Err(EspeakFailure::InvalidProvider);
            }
        }
        let data_root = exact(data_root)?;
        if !data_root.is_dir()
            || data_root.file_name().and_then(|p| p.to_str()) != Some("espeak-ng-data")
        {
            return Err(EspeakFailure::InvalidProvider);
        }
        let mut dependencies = dependencies
            .iter()
            .map(|p| regular(p))
            .collect::<Result<Vec<_>, _>>()?;
        dependencies.sort();
        if dependencies.windows(2).any(|p| p[0] == p[1]) {
            return Err(EspeakFailure::InvalidProvider);
        }
        let engines: Vec<_> = dependencies
            .iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|p| p.to_str())
                    .is_some_and(|name| name.starts_with("libespeak-ng.so."))
            })
            .collect();
        let [engine] = engines.as_slice() else {
            return Err(EspeakFailure::InvalidProvider);
        };
        let engine = (*engine).clone();
        let directory = engine.parent().ok_or(EspeakFailure::InvalidProvider)?;
        if dependencies
            .iter()
            .any(|path| path.parent() != Some(directory))
            || fs::canonicalize(directory.join("libespeak-ng.so.1"))
                .map_err(|_| EspeakFailure::InvalidProvider)?
                != engine
        {
            return Err(EspeakFailure::InvalidProvider);
        }
        let mut files = vec![("executable".to_string(), executable.clone())];
        for dependency in &dependencies {
            files.push((
                format!(
                    "library/{}",
                    dependency
                        .file_name()
                        .and_then(|p| p.to_str())
                        .ok_or(EspeakFailure::InvalidProvider)?
                ),
                dependency.clone(),
            ));
        }
        let mut remaining = MAXIMUM_FILES;
        collect(&data_root, &data_root, 0, &mut remaining, &mut files)?;
        files.sort_by(|a, b| a.0.cmp(&b.0));
        let mut hash = Sha256::new();
        hash.update(
            b"conduit.espeak/provider-closure@1\0stdout-stdin-utf8-deterministic-default-rate\0",
        );
        field(&mut hash, voice.as_bytes());
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 8192];
        for (name, path) in &files {
            let mut file = fs::File::open(path).map_err(|_| EspeakFailure::InvalidProvider)?;
            let size = file
                .metadata()
                .map_err(|_| EspeakFailure::InvalidProvider)?
                .len();
            bytes = bytes
                .checked_add(size)
                .ok_or(EspeakFailure::InvalidProvider)?;
            if size == 0 || bytes > MAXIMUM_BYTES {
                return Err(EspeakFailure::InvalidProvider);
            }
            field(&mut hash, name.as_bytes());
            hash.update(size.to_le_bytes());
            let mut remaining = size;
            while remaining > 0 {
                let limit = buffer.len().min(remaining as usize);
                let count = file
                    .read(&mut buffer[..limit])
                    .map_err(|_| EspeakFailure::InvalidProvider)?;
                if count == 0 {
                    return Err(EspeakFailure::ProviderChanged);
                }
                hash.update(&buffer[..count]);
                remaining -= count as u64;
            }
            if file
                .read(&mut buffer[..1])
                .map_err(|_| EspeakFailure::InvalidProvider)?
                != 0
            {
                return Err(EspeakFailure::ProviderChanged);
            }
        }
        let digest: [u8; 32] = hash.finalize().into();
        Ok(Self {
            executable,
            data_root,
            engine,
            voice: voice.to_string(),
            dependencies,
            provider_sha256: digest.iter().map(|b| format!("{b:02x}")).collect(),
            digest,
            bytes: bytes as u32,
            items: files.len() as u32,
        })
    }
    pub fn content_requirement(&self) -> ResourceContentRequirement {
        ResourceContentRequirement {
            identity: ResourceSemanticIdentity::from_digest(self.digest),
            version: ResourceVersionIdentity::from_digest(self.digest),
            content_profile: kind_id(conduit_std_offers::ESPEAK_PROVIDER_CONTENT_PROFILE),
            maximum_bytes: self.bytes,
            maximum_items: self.items,
            retention: ResourceRetention::Boot,
            sharing: ResourceSharing::ImmutableReadMany,
            access: ResourceAccessMode::ReadPublished,
            generation_slots: 1,
            reader_leases: 1,
            publication_slots: 0,
            sensitive: false,
        }
    }
    pub(super) fn pool_id(&self) -> String {
        format!("std/espeak-ng/{}", self.provider_sha256)
    }
    pub(super) fn verify(&self) -> Result<(), EspeakFailure> {
        let current = Self::inspect(
            &self.executable,
            &self.data_root,
            &self.voice,
            &self.dependencies,
        )
        .map_err(|_| EspeakFailure::ProviderChanged)?;
        if current != *self {
            return Err(EspeakFailure::ProviderChanged);
        }
        Ok(())
    }
}
fn field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn exact(path: &Path) -> Result<PathBuf, EspeakFailure> {
    if !path.is_absolute()
        || fs::symlink_metadata(path)
            .map_err(|_| EspeakFailure::InvalidProvider)?
            .file_type()
            .is_symlink()
    {
        return Err(EspeakFailure::InvalidProvider);
    }
    let canonical = fs::canonicalize(path).map_err(|_| EspeakFailure::InvalidProvider)?;
    if canonical != path {
        return Err(EspeakFailure::InvalidProvider);
    }
    Ok(canonical)
}
fn regular(path: &Path) -> Result<PathBuf, EspeakFailure> {
    let path = exact(path)?;
    if !path.is_file() {
        return Err(EspeakFailure::InvalidProvider);
    }
    Ok(path)
}
fn collect(
    root: &Path,
    directory: &Path,
    depth: usize,
    remaining: &mut usize,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), EspeakFailure> {
    if depth > 16 {
        return Err(EspeakFailure::InvalidProvider);
    }
    for entry in fs::read_dir(directory).map_err(|_| EspeakFailure::InvalidProvider)? {
        *remaining = remaining
            .checked_sub(1)
            .ok_or(EspeakFailure::InvalidProvider)?;
        let entry = entry.map_err(|_| EspeakFailure::InvalidProvider)?;
        let kind = entry
            .file_type()
            .map_err(|_| EspeakFailure::InvalidProvider)?;
        let path = entry.path();
        if kind.is_dir() {
            collect(root, &path, depth + 1, remaining, files)?;
        } else if kind.is_file() {
            if files.len() >= MAXIMUM_FILES {
                return Err(EspeakFailure::InvalidProvider);
            }
            let name = path
                .strip_prefix(root)
                .ok()
                .and_then(|p| p.to_str())
                .ok_or(EspeakFailure::InvalidProvider)?;
            if name.len() > 512 {
                return Err(EspeakFailure::InvalidProvider);
            }
            files.push((format!("data/{name}"), path));
        } else {
            return Err(EspeakFailure::InvalidProvider);
        }
    }
    Ok(())
}
