//! Deterministic checked `pack.conduit` identity and generated lock truth.

use crate::prelude::*;
use crate::syntax::PackageSyntax;
use alloc::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CONDUIT_LOCK_SCHEMA: &str = "conduit.lock/v1";
pub const MAXIMUM_PACKAGE_CATALOG_ENTRIES: usize = 1024;
pub const MAXIMUM_PACKAGE_RESOLUTION_STEPS: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PackageVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PackageVersionRequirement {
    Exact(PackageVersion),
    Caret(PackageVersion),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedPackageRequirement {
    pub path: String,
    pub version: PackageVersionRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedPackageSource {
    pub path: String,
    pub version: PackageVersion,
    pub content_digest: [u8; 32],
    pub exports: Vec<String>,
    pub requirements: Vec<CheckedPackageRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedPackageRequirement {
    pub path: String,
    pub version: PackageVersion,
    pub content_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedPackage {
    pub path: String,
    pub version: PackageVersion,
    pub content_digest: [u8; 32],
    pub requirements: Vec<LockedPackageRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConduitLock {
    pub schema: String,
    pub roots: Vec<LockedPackageRequirement>,
    pub packages: Vec<LockedPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageResolutionError {
    SourceMismatch,
    InvalidCheckedPackage(String),
    CatalogLimitExceeded,
    ResolutionWorkExceeded,
    InvalidLock,
    InvalidVersion(String),
    DuplicatePackage {
        path: String,
        version: PackageVersion,
    },
    DuplicateContentIdentity,
    MissingPackage(String),
    NoMatchingVersion(String),
    ConflictingRequirement(String),
    DependencyCycle(Vec<String>),
    UnknownRoot(String),
}

impl core::fmt::Display for PackageResolutionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl core::error::Error for PackageResolutionError {}

impl CheckedPackageSource {
    pub fn from_syntax(
        source: &str,
        package: &PackageSyntax,
    ) -> Result<Self, PackageResolutionError> {
        let document = crate::parse_syntax_document(source);
        if !document.diagnostics.is_empty()
            || document.packages.len() != 1
            || document.packages.first() != Some(package)
        {
            return Err(PackageResolutionError::SourceMismatch);
        }
        let version = PackageVersion::parse(&package.version.text)?;
        let mut exports = package
            .exports
            .iter()
            .map(|export| export.text.clone())
            .collect::<Vec<_>>();
        exports.sort();
        let mut requirements = package
            .requirements
            .iter()
            .map(|requirement| {
                Ok(CheckedPackageRequirement {
                    path: requirement.path.text.clone(),
                    version: PackageVersionRequirement::parse(
                        &requirement.version_requirement.text,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, PackageResolutionError>>()?;
        requirements.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(Self {
            path: package.path.text.clone(),
            version,
            content_digest: Sha256::digest(source.as_bytes()).into(),
            exports,
            requirements,
        })
    }

    pub fn validate(&self) -> Result<(), PackageResolutionError> {
        if !crate::surface_lex::is_operation(&self.path)
            || !self.path.contains('/')
            || self.content_digest == [0; 32]
            || self.exports.len() > crate::MAXIMUM_PACKAGE_EXPORTS
            || self.requirements.len() > crate::MAXIMUM_PACKAGE_REQUIREMENTS
            || !strictly_ordered(&self.exports, |name| name)
            || self
                .exports
                .iter()
                .any(|name| !crate::surface_lex::is_name(name))
            || !strictly_ordered(&self.requirements, |requirement| &requirement.path)
            || self.requirements.iter().any(|requirement| {
                !crate::surface_lex::is_operation(&requirement.path)
                    || !requirement.path.contains('/')
            })
        {
            return Err(PackageResolutionError::InvalidCheckedPackage(
                self.path.clone(),
            ));
        }
        Ok(())
    }
}

impl ConduitLock {
    /// Re-resolves the finite catalog and requires byte-for-byte semantic lock
    /// truth. A deserialized lock is evidence only after this check succeeds.
    pub fn validate_against(
        &self,
        catalog: &[CheckedPackageSource],
    ) -> Result<(), PackageResolutionError> {
        if self.schema != CONDUIT_LOCK_SCHEMA {
            return Err(PackageResolutionError::InvalidLock);
        }
        let roots = self
            .roots
            .iter()
            .map(|root| (root.path.as_str(), root.version))
            .collect::<Vec<_>>();
        let expected = resolve_package_lock(catalog, &roots)?;
        if self != &expected {
            return Err(PackageResolutionError::InvalidLock);
        }
        Ok(())
    }
}

impl PackageVersion {
    pub fn parse(source: &str) -> Result<Self, PackageResolutionError> {
        let mut parts = source.split('.');
        let major = number(parts.next(), source)?;
        let minor = number(parts.next(), source)?;
        let patch = number(parts.next(), source)?;
        if parts.next().is_some() {
            return Err(PackageResolutionError::InvalidVersion(source.into()));
        }
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl PackageVersionRequirement {
    pub fn parse(source: &str) -> Result<Self, PackageResolutionError> {
        let (caret, source) = source
            .strip_prefix('^')
            .map_or((false, source), |source| (true, source));
        let mut parts = source.split('.');
        let major = number(parts.next(), source)?;
        let minor = number(parts.next(), source)?;
        let patch = match parts.next() {
            Some(value) => number(Some(value), source)?,
            None => 0,
        };
        if parts.next().is_some() {
            return Err(PackageResolutionError::InvalidVersion(source.into()));
        }
        let version = PackageVersion {
            major,
            minor,
            patch,
        };
        Ok(if caret {
            Self::Caret(version)
        } else {
            Self::Exact(version)
        })
    }

    pub fn accepts(&self, candidate: PackageVersion) -> bool {
        match self {
            Self::Exact(version) => candidate == *version,
            Self::Caret(minimum) => {
                candidate >= *minimum
                    && if minimum.major > 0 {
                        candidate.major == minimum.major
                    } else if minimum.minor > 0 {
                        candidate.major == 0 && candidate.minor == minimum.minor
                    } else {
                        candidate.major == 0
                            && candidate.minor == 0
                            && candidate.patch == minimum.patch
                    }
            }
        }
    }
}

/// Resolves exact roots from a finite local catalog. It performs no fetch,
/// import-time execution, credential lookup, or ambient path discovery.
pub fn resolve_package_lock(
    catalog: &[CheckedPackageSource],
    roots: &[(&str, PackageVersion)],
) -> Result<ConduitLock, PackageResolutionError> {
    if roots.len() > MAXIMUM_PACKAGE_CATALOG_ENTRIES {
        return Err(PackageResolutionError::CatalogLimitExceeded);
    }
    let index = index_catalog(catalog)?;
    let mut canonical_roots = BTreeMap::new();
    for (path, version) in roots {
        if canonical_roots
            .insert((*path).to_string(), *version)
            .is_some_and(|prior| prior != *version)
        {
            return Err(PackageResolutionError::ConflictingRequirement(
                (*path).into(),
            ));
        }
    }
    let mut pending = Vec::new();
    for (path, version) in &canonical_roots {
        if exact(&index, path, *version).is_none() {
            return Err(PackageResolutionError::UnknownRoot(path.clone()));
        }
        pending.push(PendingRequirement {
            path: path.clone(),
            version: PackageVersionRequirement::Exact(*version),
        });
    }
    let mut remaining_steps = MAXIMUM_PACKAGE_RESOLUTION_STEPS;
    let selected = solve(&index, pending, BTreeMap::new(), &mut remaining_steps)?;
    reject_cycles(&index, &selected)?;
    let mut root_locks = canonical_roots
        .iter()
        .map(|(path, version)| {
            reference(exact(&index, path, *version).expect("validated exact root"))
        })
        .collect::<Vec<_>>();
    root_locks.sort_by(|left, right| left.path.cmp(&right.path));

    let mut packages = Vec::with_capacity(selected.len());
    for (path, version) in &selected {
        let package = exact(&index, path, *version).expect("selected package remains indexed");
        let mut requirements = package
            .requirements
            .iter()
            .map(|requirement| {
                let version = selected[&requirement.path];
                reference(exact(&index, &requirement.path, version).expect("dependency selected"))
            })
            .collect::<Vec<_>>();
        requirements.sort_by(|left, right| left.path.cmp(&right.path));
        packages.push(LockedPackage {
            path: path.clone(),
            version: *version,
            content_digest: package.content_digest,
            requirements,
        });
    }
    Ok(ConduitLock {
        schema: CONDUIT_LOCK_SCHEMA.into(),
        roots: root_locks,
        packages,
    })
}

type Catalog<'a> = BTreeMap<String, Vec<&'a CheckedPackageSource>>;

fn index_catalog(catalog: &[CheckedPackageSource]) -> Result<Catalog<'_>, PackageResolutionError> {
    if catalog.len() > MAXIMUM_PACKAGE_CATALOG_ENTRIES {
        return Err(PackageResolutionError::CatalogLimitExceeded);
    }
    let mut index = Catalog::new();
    let mut content = BTreeSet::new();
    for package in catalog {
        package.validate()?;
        if !content.insert(package.content_digest) {
            return Err(PackageResolutionError::DuplicateContentIdentity);
        }
        let versions = index.entry(package.path.clone()).or_default();
        if versions
            .iter()
            .any(|candidate| candidate.version == package.version)
        {
            return Err(PackageResolutionError::DuplicatePackage {
                path: package.path.clone(),
                version: package.version,
            });
        }
        versions.push(package);
    }
    for versions in index.values_mut() {
        versions.sort_by_key(|package| package.version);
    }
    Ok(index)
}

fn strictly_ordered<T, K: Ord + ?Sized>(values: &[T], key: impl Fn(&T) -> &K) -> bool {
    values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}

#[derive(Clone)]
struct PendingRequirement {
    path: String,
    version: PackageVersionRequirement,
}

fn solve(
    index: &Catalog<'_>,
    mut pending: Vec<PendingRequirement>,
    selected: BTreeMap<String, PackageVersion>,
    remaining_steps: &mut usize,
) -> Result<BTreeMap<String, PackageVersion>, PackageResolutionError> {
    *remaining_steps = remaining_steps
        .checked_sub(1)
        .ok_or(PackageResolutionError::ResolutionWorkExceeded)?;
    if pending.is_empty() {
        return Ok(selected);
    }
    pending.sort_by(|left, right| (&left.path, &left.version).cmp(&(&right.path, &right.version)));
    let requirement = pending.remove(0);
    if let Some(existing) = selected.get(&requirement.path) {
        if requirement.version.accepts(*existing) {
            return solve(index, pending, selected, remaining_steps);
        }
        return Err(PackageResolutionError::ConflictingRequirement(
            requirement.path,
        ));
    }
    let versions = index
        .get(&requirement.path)
        .ok_or_else(|| PackageResolutionError::MissingPackage(requirement.path.clone()))?;
    let candidates = versions
        .iter()
        .rev()
        .filter(|candidate| requirement.version.accepts(candidate.version))
        .copied()
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(PackageResolutionError::NoMatchingVersion(requirement.path));
    }
    let mut last = None;
    for candidate in candidates {
        let mut branch_selected = selected.clone();
        branch_selected.insert(candidate.path.clone(), candidate.version);
        let mut branch_pending = pending.clone();
        branch_pending.extend(
            candidate
                .requirements
                .iter()
                .map(|dependency| PendingRequirement {
                    path: dependency.path.clone(),
                    version: dependency.version.clone(),
                }),
        );
        match solve(index, branch_pending, branch_selected, remaining_steps) {
            Ok(selected) => return Ok(selected),
            Err(error) => last = Some(error),
        }
    }
    Err(
        last.unwrap_or(PackageResolutionError::ConflictingRequirement(
            requirement.path,
        )),
    )
}

fn reject_cycles(
    index: &Catalog<'_>,
    selected: &BTreeMap<String, PackageVersion>,
) -> Result<(), PackageResolutionError> {
    let mut visited = BTreeSet::new();
    for path in selected.keys() {
        visit(path, index, selected, &mut visited, &mut Vec::new())?;
    }
    Ok(())
}

fn visit(
    path: &str,
    index: &Catalog<'_>,
    selected: &BTreeMap<String, PackageVersion>,
    visited: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
) -> Result<(), PackageResolutionError> {
    if let Some(position) = stack.iter().position(|candidate| candidate == path) {
        let mut cycle = stack[position..].to_vec();
        cycle.push(path.into());
        return Err(PackageResolutionError::DependencyCycle(cycle));
    }
    if !visited.insert(path.into()) {
        return Ok(());
    }
    stack.push(path.into());
    let package = exact(index, path, selected[path]).expect("selected package remains indexed");
    for dependency in &package.requirements {
        visit(&dependency.path, index, selected, visited, stack)?;
    }
    stack.pop();
    Ok(())
}

fn exact<'a>(
    index: &'a Catalog<'a>,
    path: &str,
    version: PackageVersion,
) -> Option<&'a CheckedPackageSource> {
    index
        .get(path)?
        .iter()
        .find(|candidate| candidate.version == version)
        .copied()
}

fn reference(package: &CheckedPackageSource) -> LockedPackageRequirement {
    LockedPackageRequirement {
        path: package.path.clone(),
        version: package.version,
        content_digest: package.content_digest,
    }
}

fn number(value: Option<&str>, source: &str) -> Result<u32, PackageResolutionError> {
    let value = value.ok_or_else(|| PackageResolutionError::InvalidVersion(source.into()))?;
    if value.is_empty() || (value != "0" && value.starts_with('0')) {
        return Err(PackageResolutionError::InvalidVersion(source.into()));
    }
    value
        .parse()
        .map_err(|_| PackageResolutionError::InvalidVersion(source.into()))
}
