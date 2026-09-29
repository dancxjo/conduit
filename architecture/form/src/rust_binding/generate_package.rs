use super::{
    generate_rust_bindings, RustBindingGenerationError, RustBindingModule, RustBindingOptions,
};
use crate::{
    check_package_bundle, CheckedPackageBundle, CheckedPackageSource, ConduitLock,
    PackageCheckError, PackageMemberSource, PackageResolutionError, PackageSyntax, StartupCatalog,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockedRustBindingGenerationError {
    InvalidLock(PackageResolutionError),
    SourceNotLocked,
    Package(PackageCheckError),
    Generation(RustBindingGenerationError),
}

pub struct LockedPackageRustBindingInput<'a> {
    pub bundle: &'a CheckedPackageBundle,
    pub manifest_source: &'a str,
    pub manifest: &'a PackageSyntax,
    pub member_sources: &'a [PackageMemberSource<'a>],
    pub lock: &'a ConduitLock,
    pub locked_catalog: &'a [CheckedPackageSource],
    pub semantic_catalog: &'a StartupCatalog,
}

/// Generates bindings only after the supplied source bundle and lock have both
/// been revalidated. The operation is deterministic, finite, and performs no
/// network or build-script work.
pub fn generate_locked_package_rust_bindings(
    input: LockedPackageRustBindingInput<'_>,
    options: &RustBindingOptions,
) -> Result<RustBindingModule, LockedRustBindingGenerationError> {
    input
        .lock
        .validate_against(input.locked_catalog)
        .map_err(LockedRustBindingGenerationError::InvalidLock)?;
    if !input
        .locked_catalog
        .iter()
        .any(|package| package == &input.bundle.package)
        || !input.lock.roots.iter().any(|root| {
            root.path == input.bundle.package.path
                && root.version == input.bundle.package.version
                && root.content_digest == input.bundle.package.content_digest
        })
    {
        return Err(LockedRustBindingGenerationError::SourceNotLocked);
    }
    let checked = check_package_bundle(
        input.bundle,
        input.manifest_source,
        input.manifest,
        input.member_sources,
        input.semantic_catalog,
    )
    .map_err(LockedRustBindingGenerationError::Package)?;
    generate_rust_bindings(&checked.native_types, options)
        .map_err(LockedRustBindingGenerationError::Generation)
}
