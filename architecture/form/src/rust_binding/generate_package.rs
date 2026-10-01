use super::{
    generate_rust_bindings, RustBindingGenerationError, RustBindingModule, RustBindingOptions,
};
use crate::prelude::*;
use crate::{
    check_package_bundle, CheckedPackageBundle, ConduitLock, PackageBundleError, PackageCheckError,
    PackageExportCatalog, PackageMemberSource, PackageResolutionError, PackageSyntax,
    StartupCatalog, SyntaxCheckDiagnostic,
};
use alloc::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockedRustBindingGenerationError {
    InvalidLock(PackageResolutionError),
    SourceNotLocked,
    MissingLockedSource(String),
    DuplicateLockedSource(String),
    UnexpectedLockedSource(String),
    Bundle(PackageBundleError),
    NativeType(SyntaxCheckDiagnostic),
    Package(PackageCheckError),
    Generation(RustBindingGenerationError),
}

#[derive(Clone, Copy)]
pub struct LockedPackageBindingSource<'a> {
    pub bundle: &'a CheckedPackageBundle,
    pub manifest_source: &'a str,
    pub manifest: &'a PackageSyntax,
    pub member_sources: &'a [PackageMemberSource<'a>],
}

pub struct LockedPackageRustBindingInput<'a> {
    pub root: LockedPackageBindingSource<'a>,
    /// Exact source for every package named by `lock.packages`, including root.
    pub locked_sources: &'a [LockedPackageBindingSource<'a>],
    pub lock: &'a ConduitLock,
}

/// Generates bindings only after every source bundle named by the lock has
/// been revalidated. Dependency Types are installed from those checked sources
/// in dependency order; callers cannot inject ambient semantic meaning.
pub fn generate_locked_package_rust_bindings(
    input: LockedPackageRustBindingInput<'_>,
    options: &RustBindingOptions,
) -> Result<RustBindingModule, LockedRustBindingGenerationError> {
    let mut sources = BTreeMap::new();
    let mut package_catalog = Vec::with_capacity(input.locked_sources.len());
    for source in input.locked_sources {
        let path = source.bundle.package.path.clone();
        if sources.insert(path.clone(), *source).is_some() {
            return Err(LockedRustBindingGenerationError::DuplicateLockedSource(
                path,
            ));
        }
        source
            .bundle
            .validate_against(
                source.manifest_source,
                source.manifest,
                source.member_sources,
            )
            .map_err(LockedRustBindingGenerationError::Bundle)?;
        package_catalog.push(source.bundle.package.clone());
    }
    if !input.lock.roots.iter().any(|root| {
        root.path == input.root.bundle.package.path
            && root.version == input.root.bundle.package.version
            && root.content_digest == input.root.bundle.package.content_digest
    }) {
        return Err(LockedRustBindingGenerationError::SourceNotLocked);
    }
    for package in &input.lock.packages {
        if !sources.contains_key(&package.path) {
            return Err(LockedRustBindingGenerationError::MissingLockedSource(
                package.path.clone(),
            ));
        }
    }
    for path in sources.keys() {
        if !input
            .lock
            .packages
            .iter()
            .any(|package| &package.path == path)
        {
            return Err(LockedRustBindingGenerationError::UnexpectedLockedSource(
                path.clone(),
            ));
        }
    }
    input
        .lock
        .validate_against(&package_catalog)
        .map_err(LockedRustBindingGenerationError::InvalidLock)?;
    let locked_packages = input
        .lock
        .packages
        .iter()
        .map(|package| (package.path.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let mut semantic_catalog = StartupCatalog::new();
    let mut installed = BTreeSet::new();
    let mut generated_types = Vec::new();
    install_dependencies(
        &input.root.bundle.package.path,
        &sources,
        &locked_packages,
        &mut installed,
        &mut semantic_catalog,
        &mut generated_types,
        false,
    )?;
    let checked = check_package_bundle(
        input.root.bundle,
        input.root.manifest_source,
        input.root.manifest,
        input.root.member_sources,
        &semantic_catalog,
    )
    .map_err(LockedRustBindingGenerationError::Package)?;
    generated_types.extend(checked.native_types);
    generate_rust_bindings(&generated_types, options)
        .map_err(LockedRustBindingGenerationError::Generation)
}

fn install_dependencies<'a>(
    path: &str,
    sources: &BTreeMap<String, LockedPackageBindingSource<'a>>,
    locked_packages: &BTreeMap<&str, &crate::LockedPackage>,
    installed: &mut BTreeSet<String>,
    catalog: &mut StartupCatalog,
    generated_types: &mut Vec<crate::CheckedNativeType>,
    install_current: bool,
) -> Result<(), LockedRustBindingGenerationError> {
    if installed.contains(path) {
        return Ok(());
    }
    let package = locked_packages
        .get(path)
        .ok_or_else(|| LockedRustBindingGenerationError::MissingLockedSource(path.into()))?;
    for requirement in &package.requirements {
        install_dependencies(
            &requirement.path,
            sources,
            locked_packages,
            installed,
            catalog,
            generated_types,
            true,
        )?;
    }
    if install_current {
        let source = sources
            .get(path)
            .ok_or_else(|| LockedRustBindingGenerationError::MissingLockedSource(path.into()))?;
        let shipped = PackageExportCatalog::from_bundle(
            source.bundle,
            source.manifest_source,
            source.manifest,
            source.member_sources,
        )
        .map_err(LockedRustBindingGenerationError::Bundle)?
        .install_shipped_types(catalog)
        .map_err(LockedRustBindingGenerationError::NativeType)?;
        generated_types.extend(shipped);
    }
    installed.insert(path.into());
    Ok(())
}
