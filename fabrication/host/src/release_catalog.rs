//! Installed release-catalog resolution for a local or air-gapped mirror.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub const RELEASE_CATALOG_SCHEMA: &str = "conduit.release/catalog@1";
pub const MAXIMUM_RELEASE_CATALOG_BYTES: u64 = 128 * 1024;
pub const MAXIMUM_RELEASE_CATALOG_ENTRIES: usize = 64;
pub const MAXIMUM_RELEASE_MANIFEST_BYTES: u64 = 256 * 1024;
const MAXIMUM_RELEASE_TEXT_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseArtifactDescriptor {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCatalogEntry {
    pub target_id: String,
    pub package_id: String,
    pub output: String,
    pub builder_adapter: String,
    pub deployment_adapter: String,
    pub manifest: ReleaseArtifactDescriptor,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCatalog {
    pub schema: String,
    pub generation: u64,
    pub catalog_id: String,
    pub entries: Vec<ReleaseCatalogEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseCatalogRefusal {
    CatalogUnavailable,
    CatalogBound,
    CatalogMalformed,
    StaleCatalog,
    CatalogIdentityMismatch,
    UnknownTarget,
    CatalogMismatch,
    ArtifactUnavailable,
    ArtifactBound,
    ArtifactContentMismatch,
    CacheUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseTargetProfile<'a> {
    pub target_id: &'a str,
    pub package_id: &'a str,
    pub output: &'a str,
    pub builder_adapter: &'a str,
    pub deployment_adapter: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedReleaseArtifact {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub from_cache: bool,
}

#[derive(Serialize)]
struct CatalogIdentity<'a> {
    schema: &'a str,
    generation: u64,
    entries: &'a [ReleaseCatalogEntry],
}

impl ReleaseCatalog {
    pub fn open_local(
        catalog_path: &Path,
        minimum_generation: u64,
    ) -> Result<Self, ReleaseCatalogRefusal> {
        let bytes = bounded_read(
            catalog_path,
            MAXIMUM_RELEASE_CATALOG_BYTES,
            ReleaseCatalogRefusal::CatalogUnavailable,
            ReleaseCatalogRefusal::CatalogBound,
        )?;
        Self::open_bytes(&bytes, minimum_generation)
    }

    pub fn open_bytes(
        bytes: &[u8],
        minimum_generation: u64,
    ) -> Result<Self, ReleaseCatalogRefusal> {
        if bytes.is_empty() || bytes.len() as u64 > MAXIMUM_RELEASE_CATALOG_BYTES {
            return Err(ReleaseCatalogRefusal::CatalogBound);
        }
        let catalog: Self =
            serde_json::from_slice(bytes).map_err(|_| ReleaseCatalogRefusal::CatalogMalformed)?;
        catalog.validate(minimum_generation)?;
        Ok(catalog)
    }

    pub fn validate(&self, minimum_generation: u64) -> Result<(), ReleaseCatalogRefusal> {
        if self.schema != RELEASE_CATALOG_SCHEMA
            || self.generation <= minimum_generation
            || !valid_sha256(&self.catalog_id)
            || self.entries.is_empty()
            || self.entries.len() > MAXIMUM_RELEASE_CATALOG_ENTRIES
        {
            return Err(ReleaseCatalogRefusal::StaleCatalog);
        }
        for (index, entry) in self.entries.iter().enumerate() {
            if !bounded(&entry.target_id)
                || !bounded(&entry.package_id)
                || !bounded(&entry.output)
                || !bounded(&entry.builder_adapter)
                || !bounded(&entry.deployment_adapter)
                || !valid_artifact(&entry.manifest, MAXIMUM_RELEASE_MANIFEST_BYTES)
                || self.entries[..index]
                    .iter()
                    .any(|prior| prior.target_id == entry.target_id)
            {
                return Err(ReleaseCatalogRefusal::CatalogMalformed);
            }
        }
        let identity = serde_json::to_vec(&CatalogIdentity {
            schema: &self.schema,
            generation: self.generation,
            entries: &self.entries,
        })
        .map_err(|_| ReleaseCatalogRefusal::CatalogMalformed)?;
        if sha256(&identity) != self.catalog_id {
            return Err(ReleaseCatalogRefusal::CatalogIdentityMismatch);
        }
        Ok(())
    }

    pub fn resolve<'a>(
        &'a self,
        profile: &ReleaseTargetProfile<'_>,
    ) -> Result<&'a ReleaseCatalogEntry, ReleaseCatalogRefusal> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.target_id == profile.target_id)
            .ok_or(ReleaseCatalogRefusal::UnknownTarget)?;
        if entry.package_id != profile.package_id
            || entry.output != profile.output
            || entry.builder_adapter != profile.builder_adapter
            || entry.deployment_adapter != profile.deployment_adapter
        {
            return Err(ReleaseCatalogRefusal::CatalogMismatch);
        }
        Ok(entry)
    }
}

/// Verify and cache one selected immutable artifact from an offline mirror.
/// No unselected entry or Body-specific value is read or copied.
pub fn acquire_local_release_artifact(
    mirror_root: &Path,
    descriptor: &ReleaseArtifactDescriptor,
    maximum_bytes: u64,
    cache_directory: &Path,
) -> Result<ResolvedReleaseArtifact, ReleaseCatalogRefusal> {
    acquire_release_artifact_with(descriptor, maximum_bytes, cache_directory, || {
        let source = mirror_root.join(&descriptor.path);
        bounded_read(
            &source,
            descriptor.bytes,
            ReleaseCatalogRefusal::ArtifactUnavailable,
            ReleaseCatalogRefusal::ArtifactBound,
        )
    })
}

pub fn acquire_release_artifact_with(
    descriptor: &ReleaseArtifactDescriptor,
    maximum_bytes: u64,
    cache_directory: &Path,
    acquire: impl FnOnce() -> Result<Vec<u8>, ReleaseCatalogRefusal>,
) -> Result<ResolvedReleaseArtifact, ReleaseCatalogRefusal> {
    if !valid_artifact(descriptor, maximum_bytes) {
        return Err(ReleaseCatalogRefusal::ArtifactBound);
    }
    fs::create_dir_all(cache_directory).map_err(|_| ReleaseCatalogRefusal::CacheUnavailable)?;
    let cache_path = cache_directory.join(&descriptor.sha256[7..]);
    if cache_path.exists() {
        verify_file(&cache_path, descriptor)?;
        return Ok(ResolvedReleaseArtifact {
            path: cache_path,
            sha256: descriptor.sha256.clone(),
            bytes: descriptor.bytes,
            from_cache: true,
        });
    }
    let bytes = acquire()?;
    verify_bytes(&bytes, descriptor)?;
    let temporary = cache_path.with_extension("conduit-partial");
    if temporary.exists() {
        fs::remove_file(&temporary).map_err(|_| ReleaseCatalogRefusal::CacheUnavailable)?;
    }
    write_bounded(&bytes, &temporary, descriptor.bytes)?;
    verify_file(&temporary, descriptor)?;
    fs::rename(&temporary, &cache_path).map_err(|_| ReleaseCatalogRefusal::CacheUnavailable)?;
    Ok(ResolvedReleaseArtifact {
        path: cache_path,
        sha256: descriptor.sha256.clone(),
        bytes: descriptor.bytes,
        from_cache: false,
    })
}

fn verify_bytes(
    bytes: &[u8],
    descriptor: &ReleaseArtifactDescriptor,
) -> Result<(), ReleaseCatalogRefusal> {
    if bytes.len() as u64 != descriptor.bytes {
        return Err(ReleaseCatalogRefusal::ArtifactBound);
    }
    if sha256(bytes) != descriptor.sha256 {
        return Err(ReleaseCatalogRefusal::ArtifactContentMismatch);
    }
    Ok(())
}

fn valid_artifact(descriptor: &ReleaseArtifactDescriptor, maximum_bytes: u64) -> bool {
    bounded_relative_path(&descriptor.path)
        && descriptor.bytes > 0
        && descriptor.bytes <= maximum_bytes
        && valid_sha256(&descriptor.sha256)
}

fn bounded_relative_path(value: &str) -> bool {
    if !bounded(value) {
        return false;
    }
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAXIMUM_RELEASE_TEXT_BYTES
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn verify_file(
    path: &Path,
    descriptor: &ReleaseArtifactDescriptor,
) -> Result<(), ReleaseCatalogRefusal> {
    let bytes = bounded_read(
        path,
        descriptor.bytes,
        ReleaseCatalogRefusal::ArtifactUnavailable,
        ReleaseCatalogRefusal::ArtifactBound,
    )?;
    if bytes.len() as u64 != descriptor.bytes {
        return Err(ReleaseCatalogRefusal::ArtifactBound);
    }
    if sha256(&bytes) != descriptor.sha256 {
        return Err(ReleaseCatalogRefusal::ArtifactContentMismatch);
    }
    Ok(())
}

fn bounded_read(
    path: &Path,
    maximum: u64,
    unavailable: ReleaseCatalogRefusal,
    bound: ReleaseCatalogRefusal,
) -> Result<Vec<u8>, ReleaseCatalogRefusal> {
    let metadata = fs::metadata(path).map_err(|_| unavailable)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(bound);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|_| unavailable)?;
    Ok(bytes)
}

fn write_bounded(
    bytes: &[u8],
    destination: &Path,
    maximum: u64,
) -> Result<(), ReleaseCatalogRefusal> {
    if bytes.len() as u64 > maximum {
        return Err(ReleaseCatalogRefusal::ArtifactBound);
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| ReleaseCatalogRefusal::CacheUnavailable)?;
    output
        .write_all(bytes)
        .map_err(|_| ReleaseCatalogRefusal::CacheUnavailable)?;
    output
        .sync_all()
        .map_err(|_| ReleaseCatalogRefusal::CacheUnavailable)
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(path: &str, bytes: &[u8]) -> ReleaseArtifactDescriptor {
        ReleaseArtifactDescriptor {
            path: path.into(),
            bytes: bytes.len() as u64,
            sha256: sha256(bytes),
        }
    }

    fn catalog(entries: Vec<ReleaseCatalogEntry>) -> ReleaseCatalog {
        let mut catalog = ReleaseCatalog {
            schema: RELEASE_CATALOG_SCHEMA.into(),
            generation: 7,
            catalog_id: String::new(),
            entries,
        };
        catalog.catalog_id = sha256(
            &serde_json::to_vec(&CatalogIdentity {
                schema: &catalog.schema,
                generation: catalog.generation,
                entries: &catalog.entries,
            })
            .unwrap(),
        );
        catalog
    }

    #[test]
    fn selected_offline_artifact_is_verified_and_second_request_uses_exact_cache() {
        let root =
            std::env::temp_dir().join(format!("conduit-release-catalog-{}", std::process::id()));
        let mirror = root.join("mirror");
        let cache = root.join("cache");
        fs::create_dir_all(&mirror).unwrap();
        let selected = b"reviewed selected manifest";
        let unrelated_secret = b"invitation-secret-must-not-be-cached";
        fs::write(mirror.join("selected.json"), selected).unwrap();
        fs::write(mirror.join("unselected.bin"), unrelated_secret).unwrap();
        let selected_descriptor = artifact("selected.json", selected);
        let release = catalog(vec![ReleaseCatalogEntry {
            target_id: "std/x86_64/computer".into(),
            package_id: "conduit-host-hosted@1".into(),
            output: "native-bundle".into(),
            builder_adapter: "hosted/build@1".into(),
            deployment_adapter: "hosted/install@1".into(),
            manifest: selected_descriptor.clone(),
        }]);
        assert_eq!(release.validate(6), Ok(()));
        let entry = release
            .resolve(&ReleaseTargetProfile {
                target_id: "std/x86_64/computer",
                package_id: "conduit-host-hosted@1",
                output: "native-bundle",
                builder_adapter: "hosted/build@1",
                deployment_adapter: "hosted/install@1",
            })
            .unwrap();
        let first = acquire_local_release_artifact(
            &mirror,
            &entry.manifest,
            MAXIMUM_RELEASE_MANIFEST_BYTES,
            &cache,
        )
        .unwrap();
        assert!(!first.from_cache);
        fs::remove_file(mirror.join("selected.json")).unwrap();
        let second = acquire_local_release_artifact(
            &mirror,
            &entry.manifest,
            MAXIMUM_RELEASE_MANIFEST_BYTES,
            &cache,
        )
        .unwrap();
        assert!(second.from_cache);
        assert_eq!(fs::read(second.path).unwrap(), selected);
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
        assert!(!fs::read_dir(&cache).unwrap().any(|item| {
            fs::read(item.unwrap().path())
                .unwrap()
                .windows(unrelated_secret.len())
                .any(|window| window == unrelated_secret)
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_catalog_cannot_relabel_old_artifact_or_escape_mirror() {
        let manifest = artifact("manifest.json", b"reviewed");
        let mut release = catalog(vec![ReleaseCatalogEntry {
            target_id: "std/x86_64/computer".into(),
            package_id: "conduit-host-hosted@1".into(),
            output: "native-bundle".into(),
            builder_adapter: "hosted/build@1".into(),
            deployment_adapter: "hosted/install@1".into(),
            manifest,
        }]);
        release.entries[0].target_id = "std/aarch64/macos-computer".into();
        assert_eq!(
            release.validate(6),
            Err(ReleaseCatalogRefusal::CatalogIdentityMismatch)
        );
        release.entries[0].manifest.path = "../outside".into();
        assert_eq!(
            release.validate(6),
            Err(ReleaseCatalogRefusal::CatalogMalformed)
        );
    }
}
