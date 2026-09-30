//! Bounded manifests and deterministic locks for Conduit source packages.
//!
//! Package acquisition is build-time work. These values contain no runtime
//! authority, credential, install hook, or authored import syntax.

use crate::{acquire_release_artifact_with, ReleaseArtifactDescriptor, ReleaseCatalogRefusal};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

pub const SOURCE_PACKAGE_MANIFEST_SCHEMA: &str = "conduit.source-package/manifest@1";
pub const SOURCE_PACKAGE_LOCK_SCHEMA: &str = "conduit.source-package/lock@1";
pub const MAXIMUM_SOURCE_PACKAGE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAXIMUM_SOURCE_PACKAGES: usize = 128;
pub const MAXIMUM_PACKAGE_EXPORTS: usize = 256;
pub const MAXIMUM_PACKAGE_DEPENDENCIES: usize = 64;
pub const MAXIMUM_PACKAGE_CONTENT_FILES: usize = 512;
const MAXIMUM_PACKAGE_TEXT_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceExportKind {
    Form,
    Type,
    SemanticCatalog,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePackageExport {
    pub public_name: String,
    pub source_path: String,
    pub kind: SourceExportKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePackageRequirement {
    pub package_id: String,
    pub version_requirement: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePackageManifest {
    pub schema: String,
    pub package_id: String,
    pub package_version: String,
    pub language_compatibility: String,
    pub source_roots: Vec<String>,
    pub exports: Vec<SourcePackageExport>,
    pub dependencies: Vec<SourcePackageRequirement>,
    pub content_files: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum SourcePackageCarrier {
    LocalWorkspace {
        relative_path: String,
    },
    Archive {
        registry_id: String,
        relative_path: String,
    },
    Git {
        repository: String,
        commit: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedSourceDependency {
    pub package_id: String,
    pub package_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedSourcePackage {
    pub package_id: String,
    pub package_version: String,
    pub manifest_sha256: String,
    pub content_sha256: String,
    pub content_bytes: u64,
    pub dependencies: Vec<LockedSourceDependency>,
    pub carrier: SourcePackageCarrier,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePackageLock {
    pub schema: String,
    pub roots: Vec<String>,
    pub packages: Vec<LockedSourcePackage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourcePackageRefusal {
    InvalidSchema,
    InvalidIdentity,
    InvalidPath,
    InvalidDigest,
    BoundExceeded,
    DuplicateExport,
    DuplicateDependency,
    DuplicatePackage,
    NonCanonicalOrder,
    MissingDependency,
    RequirementMismatch,
    DependencyCycle,
    ManifestMismatch,
    ContentMismatch,
    CacheUnavailable,
}

impl SourcePackageManifest {
    pub fn validate(&self) -> Result<(), SourcePackageRefusal> {
        if self.schema != SOURCE_PACKAGE_MANIFEST_SCHEMA {
            return Err(SourcePackageRefusal::InvalidSchema);
        }
        if !identity(&self.package_id)
            || !identity(&self.package_version)
            || !identity(&self.language_compatibility)
        {
            return Err(SourcePackageRefusal::InvalidIdentity);
        }
        if self.source_roots.is_empty()
            || self.source_roots.len() > MAXIMUM_PACKAGE_CONTENT_FILES
            || self.exports.len() > MAXIMUM_PACKAGE_EXPORTS
            || self.dependencies.len() > MAXIMUM_PACKAGE_DEPENDENCIES
            || self.content_files.is_empty()
            || self.content_files.len() > MAXIMUM_PACKAGE_CONTENT_FILES
        {
            return Err(SourcePackageRefusal::BoundExceeded);
        }
        canonical_paths(&self.source_roots)?;
        canonical_paths(&self.content_files)?;
        for (index, export) in self.exports.iter().enumerate() {
            if !identity(&export.public_name) || !relative_path(&export.source_path) {
                return Err(SourcePackageRefusal::InvalidPath);
            }
            if self.exports[..index]
                .iter()
                .any(|prior| prior.public_name == export.public_name)
            {
                return Err(SourcePackageRefusal::DuplicateExport);
            }
        }
        if !strictly_sorted(&self.exports) {
            return Err(SourcePackageRefusal::NonCanonicalOrder);
        }
        for (index, dependency) in self.dependencies.iter().enumerate() {
            if !identity(&dependency.package_id) || !identity(&dependency.version_requirement) {
                return Err(SourcePackageRefusal::InvalidIdentity);
            }
            if self.dependencies[..index]
                .iter()
                .any(|prior| prior.package_id == dependency.package_id)
            {
                return Err(SourcePackageRefusal::DuplicateDependency);
            }
        }
        if !strictly_sorted(&self.dependencies) {
            return Err(SourcePackageRefusal::NonCanonicalOrder);
        }
        Ok(())
    }

    pub fn canonical_sha256(&self) -> Result<String, SourcePackageRefusal> {
        self.validate()?;
        serde_json::to_vec(self)
            .map(|bytes| sha256(&bytes))
            .map_err(|_| SourcePackageRefusal::ManifestMismatch)
    }
}

impl SourcePackageLock {
    pub fn validate(
        &self,
        manifests: &[SourcePackageManifest],
    ) -> Result<(), SourcePackageRefusal> {
        if self.schema != SOURCE_PACKAGE_LOCK_SCHEMA {
            return Err(SourcePackageRefusal::InvalidSchema);
        }
        if self.packages.is_empty()
            || self.packages.len() > MAXIMUM_SOURCE_PACKAGES
            || self.roots.is_empty()
            || self.roots.len() > MAXIMUM_SOURCE_PACKAGES
        {
            return Err(SourcePackageRefusal::BoundExceeded);
        }
        if !strictly_sorted(&self.roots) {
            return Err(SourcePackageRefusal::NonCanonicalOrder);
        }
        let mut by_id = BTreeMap::new();
        for package in &self.packages {
            if by_id.insert(package.package_id.as_str(), package).is_some() {
                return Err(SourcePackageRefusal::DuplicatePackage);
            }
            validate_locked_package(package)?;
        }
        if !strictly_sorted_by_id(&self.packages) {
            return Err(SourcePackageRefusal::NonCanonicalOrder);
        }
        for root in &self.roots {
            if !by_id.contains_key(root.as_str()) {
                return Err(SourcePackageRefusal::MissingDependency);
            }
        }
        if manifests.len() != self.packages.len() {
            return Err(SourcePackageRefusal::ManifestMismatch);
        }
        for manifest in manifests {
            manifest.validate()?;
            let package = by_id
                .get(manifest.package_id.as_str())
                .ok_or(SourcePackageRefusal::ManifestMismatch)?;
            if package.package_version != manifest.package_version
                || package.manifest_sha256 != manifest.canonical_sha256()?
                || package.dependencies.len() != manifest.dependencies.len()
            {
                return Err(SourcePackageRefusal::ManifestMismatch);
            }
            for (requirement, locked) in manifest.dependencies.iter().zip(&package.dependencies) {
                if requirement.package_id != locked.package_id
                    || requirement.version_requirement != locked.package_version
                {
                    return Err(SourcePackageRefusal::RequirementMismatch);
                }
                let resolved = by_id
                    .get(locked.package_id.as_str())
                    .ok_or(SourcePackageRefusal::MissingDependency)?;
                if resolved.package_version != locked.package_version {
                    return Err(SourcePackageRefusal::RequirementMismatch);
                }
            }
        }
        reject_cycles(&self.roots, &by_id)
    }

    pub fn canonical_bytes(
        &self,
        manifests: &[SourcePackageManifest],
    ) -> Result<Vec<u8>, SourcePackageRefusal> {
        self.validate(manifests)?;
        serde_json::to_vec(self).map_err(|_| SourcePackageRefusal::ManifestMismatch)
    }
}

pub fn acquire_source_package_with(
    package: &LockedSourcePackage,
    cache_directory: &Path,
    acquire: impl FnOnce() -> Result<Vec<u8>, ReleaseCatalogRefusal>,
) -> Result<PathBuf, SourcePackageRefusal> {
    validate_locked_package(package)?;
    let descriptor = ReleaseArtifactDescriptor {
        path: "source-package.bundle".into(),
        bytes: package.content_bytes,
        sha256: package.content_sha256.clone(),
    };
    acquire_release_artifact_with(
        &descriptor,
        MAXIMUM_SOURCE_PACKAGE_BYTES,
        cache_directory,
        acquire,
    )
    .map(|resolved| resolved.path)
    .map_err(map_release_refusal)
}

pub fn validate_locked_source_cache(
    lock: &SourcePackageLock,
    manifests: &[SourcePackageManifest],
    cache_directory: &Path,
) -> Result<Vec<PathBuf>, SourcePackageRefusal> {
    lock.validate(manifests)?;
    lock.packages
        .iter()
        .map(|package| {
            acquire_source_package_with(package, cache_directory, || {
                Err(ReleaseCatalogRefusal::ArtifactUnavailable)
            })
        })
        .collect()
}

fn validate_locked_package(package: &LockedSourcePackage) -> Result<(), SourcePackageRefusal> {
    if !identity(&package.package_id) || !identity(&package.package_version) {
        return Err(SourcePackageRefusal::InvalidIdentity);
    }
    if !valid_sha256(&package.manifest_sha256) || !valid_sha256(&package.content_sha256) {
        return Err(SourcePackageRefusal::InvalidDigest);
    }
    if package.content_bytes == 0
        || package.content_bytes > MAXIMUM_SOURCE_PACKAGE_BYTES
        || package.dependencies.len() > MAXIMUM_PACKAGE_DEPENDENCIES
    {
        return Err(SourcePackageRefusal::BoundExceeded);
    }
    for (index, dependency) in package.dependencies.iter().enumerate() {
        if !identity(&dependency.package_id) || !identity(&dependency.package_version) {
            return Err(SourcePackageRefusal::InvalidIdentity);
        }
        if package.dependencies[..index]
            .iter()
            .any(|prior| prior.package_id == dependency.package_id)
        {
            return Err(SourcePackageRefusal::DuplicateDependency);
        }
    }
    if !strictly_sorted(&package.dependencies) {
        return Err(SourcePackageRefusal::NonCanonicalOrder);
    }
    validate_carrier(&package.carrier)
}

fn validate_carrier(carrier: &SourcePackageCarrier) -> Result<(), SourcePackageRefusal> {
    match carrier {
        SourcePackageCarrier::LocalWorkspace {
            relative_path: path,
        } => relative_path(path)
            .then_some(())
            .ok_or(SourcePackageRefusal::InvalidPath),
        SourcePackageCarrier::Archive {
            registry_id,
            relative_path: path,
        } => {
            if identity(registry_id) && relative_path(path) {
                Ok(())
            } else {
                Err(SourcePackageRefusal::InvalidPath)
            }
        }
        SourcePackageCarrier::Git { repository, commit } => {
            let exact_commit =
                commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit());
            if safe_https_location(repository) && exact_commit {
                Ok(())
            } else {
                Err(SourcePackageRefusal::InvalidPath)
            }
        }
    }
}

fn reject_cycles<'a>(
    roots: &[String],
    packages: &BTreeMap<&'a str, &'a LockedSourcePackage>,
) -> Result<(), SourcePackageRefusal> {
    fn visit<'a>(
        id: &'a str,
        packages: &BTreeMap<&'a str, &'a LockedSourcePackage>,
        visiting: &mut Vec<&'a str>,
        visited: &mut Vec<&'a str>,
    ) -> Result<(), SourcePackageRefusal> {
        if visiting.contains(&id) {
            return Err(SourcePackageRefusal::DependencyCycle);
        }
        if visited.contains(&id) {
            return Ok(());
        }
        visiting.push(id);
        let package = packages
            .get(id)
            .ok_or(SourcePackageRefusal::MissingDependency)?;
        for dependency in &package.dependencies {
            visit(dependency.package_id.as_str(), packages, visiting, visited)?;
        }
        visiting.pop();
        visited.push(id);
        Ok(())
    }
    let mut visiting = Vec::new();
    let mut visited = Vec::new();
    for root in roots {
        visit(root, packages, &mut visiting, &mut visited)?;
    }
    if visited.len() != packages.len() {
        return Err(SourcePackageRefusal::MissingDependency);
    }
    Ok(())
}

fn canonical_paths(values: &[String]) -> Result<(), SourcePackageRefusal> {
    if !strictly_sorted(values) {
        return Err(SourcePackageRefusal::NonCanonicalOrder);
    }
    if values.iter().all(|value| relative_path(value)) {
        Ok(())
    } else {
        Err(SourcePackageRefusal::InvalidPath)
    }
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn strictly_sorted_by_id(values: &[LockedSourcePackage]) -> bool {
    values
        .windows(2)
        .all(|pair| pair[0].package_id < pair[1].package_id)
}

fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_PACKAGE_TEXT_BYTES
        && !value.chars().any(char::is_whitespace)
        && !value.contains(['?', '#'])
}

fn relative_path(value: &str) -> bool {
    if !identity(value)
        || value.contains('\\')
        || value.starts_with('~')
        || value.as_bytes().get(1) == Some(&b':')
    {
        return false;
    }
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn safe_https_location(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    !rest.is_empty() && !rest.contains(['@', '?', '#']) && identity(value)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn map_release_refusal(refusal: ReleaseCatalogRefusal) -> SourcePackageRefusal {
    match refusal {
        ReleaseCatalogRefusal::ArtifactContentMismatch => SourcePackageRefusal::ContentMismatch,
        ReleaseCatalogRefusal::CacheUnavailable | ReleaseCatalogRefusal::ArtifactUnavailable => {
            SourcePackageRefusal::CacheUnavailable
        }
        _ => SourcePackageRefusal::BoundExceeded,
    }
}
